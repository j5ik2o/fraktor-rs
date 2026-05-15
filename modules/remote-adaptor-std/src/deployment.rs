//! Std remote deployment daemon.

#[cfg(test)]
#[path = "deployment_test.rs"]
mod tests;

use std::{
  collections::BTreeMap,
  string::{String, ToString},
  sync::mpsc,
  time::Instant,
};

use fraktor_actor_core_kernel_rs::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage, props::DeployableFactoryLookupError, spawn::SpawnError},
  serialization::{SerializationExtensionShared, SerializedMessage, SerializerId},
  system::ActorSystem,
};
use fraktor_remote_core_rs::{
  address::Address,
  extension::RemoteEvent,
  transport::TransportEndpoint,
  wire::{
    RemoteDeploymentCreateFailure, RemoteDeploymentCreateRequest, RemoteDeploymentCreateSuccess,
    RemoteDeploymentFailureCode, RemoteDeploymentPdu,
  },
};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock};
use tokio::{
  sync::mpsc::{Receiver, Sender},
  task::JoinHandle,
};

use crate::association::std_instant_elapsed_millis;

const PARENT_PATH_INVALID: &str = "target parent path is invalid";

type DeploymentCorrelation = (u64, u32);
type PendingDeploymentResponses = BTreeMap<DeploymentCorrelation, mpsc::Sender<DeploymentResponse>>;

#[derive(Default)]
struct DeploymentResponseState {
  pending: PendingDeploymentResponses,
  stale:   Vec<DeploymentResponse>,
}

/// Origin-side deployment response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeploymentResponse {
  /// Create success response.
  Success(RemoteDeploymentCreateSuccess),
  /// Create failure response.
  Failure(RemoteDeploymentCreateFailure),
}

/// Dispatches deployment responses to pending origin-side requests.
#[derive(Clone)]
pub(crate) struct DeploymentResponseDispatcher {
  state: SharedLock<DeploymentResponseState>,
}

impl Default for DeploymentResponseDispatcher {
  fn default() -> Self {
    Self::new()
  }
}

impl DeploymentResponseDispatcher {
  /// Registers a pending request.
  pub(crate) fn register(&self, correlation_hi: u64, correlation_lo: u32) -> mpsc::Receiver<DeploymentResponse> {
    let (sender, receiver) = mpsc::channel();
    self.state.with_lock(|state| {
      state.pending.insert((correlation_hi, correlation_lo), sender);
    });
    receiver
  }

  /// Removes a pending request without completing it.
  pub(crate) fn cancel(&self, correlation_hi: u64, correlation_lo: u32) {
    self.state.with_lock(|state| {
      state.pending.remove(&(correlation_hi, correlation_lo));
    });
  }

  /// Completes a pending request or records the stale response.
  pub(crate) fn complete(&self, response: DeploymentResponse) {
    let key = match &response {
      | DeploymentResponse::Success(success) => (success.correlation_hi(), success.correlation_lo()),
      | DeploymentResponse::Failure(failure) => (failure.correlation_hi(), failure.correlation_lo()),
    };
    let sender = self.state.with_lock(|state| state.pending.remove(&key));
    match sender {
      | Some(sender) => {
        if sender.send(response.clone()).is_err() {
          self.record_stale(response);
        }
      },
      | None => self.record_stale(response),
    }
  }

  fn record_stale(&self, response: DeploymentResponse) {
    self.state.with_lock(|state| state.stale.push(response));
    tracing::warn!(stale_responses = self.stale_len(), "remote deployment response did not match a pending request");
  }

  /// Records a stale response for tests and diagnostics.
  pub(crate) fn stale_len(&self) -> usize {
    self.state.with_lock(|state| state.stale.len())
  }
}

impl DeploymentResponseDispatcher {
  pub(crate) fn new() -> Self {
    Self { state: SharedLock::new_with_driver::<DefaultMutex<_>>(DeploymentResponseState::default()) }
  }
}

/// Command consumed by the deployment daemon.
pub(crate) struct DeploymentDaemonCommand {
  authority: TransportEndpoint,
  request:   RemoteDeploymentCreateRequest,
}

impl DeploymentDaemonCommand {
  /// Creates a daemon command from an inbound deployment request.
  #[must_use]
  pub(crate) const fn create(authority: TransportEndpoint, request: RemoteDeploymentCreateRequest) -> Self {
    Self { authority, request }
  }
}

/// Spawns the std deployment daemon task.
pub(crate) fn spawn_deployment_daemon(
  receiver: Receiver<DeploymentDaemonCommand>,
  system: ActorSystem,
  serialization: ArcShared<SerializationExtensionShared>,
  event_sender: Sender<RemoteEvent>,
  monotonic_epoch: Instant,
) -> JoinHandle<()> {
  tokio::spawn(run_deployment_daemon(receiver, system, serialization, event_sender, monotonic_epoch))
}

async fn run_deployment_daemon(
  mut receiver: Receiver<DeploymentDaemonCommand>,
  system: ActorSystem,
  serialization: ArcShared<SerializationExtensionShared>,
  event_sender: Sender<RemoteEvent>,
  monotonic_epoch: Instant,
) {
  while let Some(command) = receiver.recv().await {
    let remote = match parse_remote_address(command.request.origin_node())
      .or_else(|| parse_remote_address(command.authority.authority()))
    {
      | Some(remote) => remote,
      | None => {
        tracing::warn!(origin_node = command.request.origin_node(), "remote deployment response address is invalid");
        continue;
      },
    };
    let pdu = handle_create_request(&system, &serialization, command.request);
    let now_ms = std_instant_elapsed_millis(monotonic_epoch);
    if let Err(error) = event_sender.send(RemoteEvent::OutboundDeployment { remote, pdu, now_ms }).await {
      tracing::warn!(?error, "remote deployment response enqueue failed");
    }
  }
}

fn handle_create_request(
  system: &ActorSystem,
  serialization: &ArcShared<SerializationExtensionShared>,
  request: RemoteDeploymentCreateRequest,
) -> RemoteDeploymentPdu {
  let correlation_hi = request.correlation_hi();
  let correlation_lo = request.correlation_lo();
  match create_child(system, serialization, &request) {
    | Ok(actor_path) => {
      RemoteDeploymentPdu::CreateSuccess(RemoteDeploymentCreateSuccess::new(correlation_hi, correlation_lo, actor_path))
    },
    | Err((code, reason)) => RemoteDeploymentPdu::CreateFailure(RemoteDeploymentCreateFailure::new(
      correlation_hi,
      correlation_lo,
      code,
      reason,
    )),
  }
}

fn create_child(
  system: &ActorSystem,
  serialization: &ArcShared<SerializationExtensionShared>,
  request: &RemoteDeploymentCreateRequest,
) -> Result<String, (RemoteDeploymentFailureCode, String)> {
  let payload = deserialize_payload(serialization, request)?;
  let props =
    system.state().deployable_props_for_payload(request.factory_id(), payload).map_err(factory_lookup_error)?;
  let parent_path = ActorPathParser::parse(request.target_parent_path())
    .map_err(|_| (RemoteDeploymentFailureCode::InvalidRequest, String::from(PARENT_PATH_INVALID)))?;
  let child = system.extended().spawn_child_at(parent_path, &props, request.child_name()).map_err(spawn_error)?;
  let path = child
    .actor_ref()
    .canonical_path()
    .ok_or_else(|| (RemoteDeploymentFailureCode::SpawnFailed, String::from("created actor path is unavailable")))?;
  Ok(path.to_canonical_uri())
}

fn deserialize_payload(
  serialization: &ArcShared<SerializationExtensionShared>,
  request: &RemoteDeploymentCreateRequest,
) -> Result<AnyMessage, (RemoteDeploymentFailureCode, String)> {
  let serialized = SerializedMessage::new(
    SerializerId::from_raw(request.serializer_id()),
    request.manifest().map(ToString::to_string),
    request.payload().to_vec(),
  );
  let payload = serialization
    .with_read(|serialization| serialization.deserialize(&serialized, None))
    .map_err(|error| (RemoteDeploymentFailureCode::DeserializationFailed, format!("{error:?}")))?;
  Ok(AnyMessage::from_erased(ArcShared::from_boxed(payload), None, false, false))
}

fn factory_lookup_error(error: DeployableFactoryLookupError) -> (RemoteDeploymentFailureCode, String) {
  match error {
    | DeployableFactoryLookupError::UnknownFactoryId(id) => (RemoteDeploymentFailureCode::UnknownFactoryId, id),
    | DeployableFactoryLookupError::FactoryRejected(error) => {
      (RemoteDeploymentFailureCode::SpawnFailed, error.reason().to_string())
    },
  }
}

fn spawn_error(error: SpawnError) -> (RemoteDeploymentFailureCode, String) {
  match error {
    | SpawnError::NameConflict(name) => (RemoteDeploymentFailureCode::DuplicateChildName, name),
    | SpawnError::InvalidProps(reason) => (RemoteDeploymentFailureCode::InvalidRequest, reason),
    | other => (RemoteDeploymentFailureCode::SpawnFailed, format!("{other:?}")),
  }
}

fn parse_remote_address(raw: &str) -> Option<Address> {
  let (system, endpoint) = raw.split_once('@')?;
  let (host, port) = endpoint.rsplit_once(':')?;
  let host = host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
  Some(Address::new(system, host, port.parse::<u16>().ok()?))
}
