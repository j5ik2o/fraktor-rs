//! Std remote deployment daemon.

#[cfg(test)]
#[path = "deployment_test.rs"]
mod tests;

use std::{
  collections::{BTreeMap, VecDeque},
  string::{String, ToString},
  sync::mpsc,
  time::Instant,
};

use fraktor_actor_core_kernel_rs::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage, props::DeployableFactoryLookupError, spawn::SpawnError},
  event::stream::{
    AddressTerminatedEvent, ClassifierKey, EventStreamEvent, EventStreamSubscriber, EventStreamSubscription,
    subscriber_handle,
  },
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

use crate::association::{parse_remote_authority, std_instant_elapsed_millis};

const PARENT_PATH_INVALID: &str = "target parent path is invalid";
const MAX_STALE_DEPLOYMENT_RESPONSES: usize = 128;

type DeploymentCorrelation = (u64, u32);
type PendingDeploymentResponses = BTreeMap<DeploymentCorrelation, PendingDeploymentResponse>;
type RemoteCreatedDeployments = BTreeMap<String, BTreeMap<DeploymentCorrelation, String>>;

struct PendingDeploymentResponse {
  authority:         String,
  started_at_millis: u64,
  sender:            mpsc::Sender<DeploymentResponse>,
}

#[derive(Default)]
struct DeploymentResponseState {
  pending:        PendingDeploymentResponses,
  remote_created: RemoteCreatedDeployments,
  stale:          VecDeque<DeploymentResponse>,
}

struct AddressTerminatedDeploymentSubscriber {
  dispatcher: DeploymentResponseDispatcher,
}

impl AddressTerminatedDeploymentSubscriber {
  fn new(dispatcher: DeploymentResponseDispatcher) -> Self {
    Self { dispatcher }
  }
}

impl EventStreamSubscriber for AddressTerminatedDeploymentSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::AddressTerminated(event) = event {
      self.dispatcher.fail_address_terminated(event);
    }
  }
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
  pub(crate) fn register(
    &self,
    correlation_hi: u64,
    correlation_lo: u32,
    authority: impl Into<String>,
    started_at_millis: u64,
  ) -> mpsc::Receiver<DeploymentResponse> {
    let (sender, receiver) = mpsc::channel();
    self.state.with_lock(|state| {
      state.pending.insert((correlation_hi, correlation_lo), PendingDeploymentResponse {
        authority: authority.into(),
        started_at_millis,
        sender,
      });
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
    let pending = self.state.with_lock(|state| {
      let pending = state.pending.remove(&key);
      if let Some(pending) = pending.as_ref()
        && let DeploymentResponse::Success(success) = &response
      {
        state
          .remote_created
          .entry(pending.authority.clone())
          .or_default()
          .insert(key, success.actor_path().to_string());
      }
      pending
    });
    match pending {
      | Some(pending) => self.send_or_record_stale(pending.sender, response),
      | None => self.record_stale(response),
    }
  }

  fn fail_address_terminated(&self, event: &AddressTerminatedEvent) {
    let pending = self.remove_address_terminated_deployments(event);
    for (correlation_hi, correlation_lo, sender) in pending {
      self.send_or_record_stale(
        sender,
        DeploymentResponse::Failure(RemoteDeploymentCreateFailure::new(
          correlation_hi,
          correlation_lo,
          RemoteDeploymentFailureCode::AddressTerminated,
          address_terminated_failure_reason(event),
        )),
      );
    }
  }

  fn remove_address_terminated_deployments(
    &self,
    event: &AddressTerminatedEvent,
  ) -> Vec<(u64, u32, mpsc::Sender<DeploymentResponse>)> {
    self.state.with_lock(|state| {
      state.remote_created.remove(event.authority());
      let keys = state
        .pending
        .iter()
        .filter_map(|(key, pending)| {
          let matches_authority = pending.authority == event.authority();
          let not_replayed_old_event = event.observed_at_millis() >= pending.started_at_millis;
          if matches_authority && not_replayed_old_event { Some(*key) } else { None }
        })
        .collect::<Vec<_>>();
      let mut pending = Vec::new();
      for (correlation_hi, correlation_lo) in keys {
        if let Some(entry) = state.pending.remove(&(correlation_hi, correlation_lo)) {
          pending.push((correlation_hi, correlation_lo, entry.sender));
        }
      }
      pending
    })
  }

  fn send_or_record_stale(&self, sender: mpsc::Sender<DeploymentResponse>, response: DeploymentResponse) {
    if let Err(error) = sender.send(response) {
      self.record_stale(error.0);
    }
  }

  fn record_stale(&self, response: DeploymentResponse) {
    self.state.with_lock(|state| {
      if state.stale.len() >= MAX_STALE_DEPLOYMENT_RESPONSES {
        state.stale.pop_front();
      }
      state.stale.push_back(response);
    });
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

pub(crate) fn subscribe_address_terminated(
  system: &ActorSystem,
  dispatcher: DeploymentResponseDispatcher,
) -> EventStreamSubscription {
  let subscriber = subscriber_handle(AddressTerminatedDeploymentSubscriber::new(dispatcher));
  system.event_stream().subscribe_with_key(ClassifierKey::AddressTerminated, &subscriber)
}

/// Command consumed by the deployment daemon.
pub(crate) struct DeploymentDaemonCommand {
  pub(crate) authority: TransportEndpoint,
  pub(crate) request:   RemoteDeploymentCreateRequest,
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
    let remote = match response_remote_for_command(&command) {
      | Some(remote) => remote,
      | None => {
        tracing::warn!(
          authority = command.authority.authority(),
          origin_node = command.request.origin_node(),
          "remote deployment response address is invalid"
        );
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

fn address_terminated_failure_reason(event: &AddressTerminatedEvent) -> String {
  format!("remote deployment target address terminated: authority={}, reason={}", event.authority(), event.reason())
}

fn response_remote_for_command(command: &DeploymentDaemonCommand) -> Option<Address> {
  let authority = parse_remote_authority(command.authority.authority());
  let origin = parse_remote_authority(command.request.origin_node());
  match (authority, origin) {
    | (Some(authority), Some(origin)) => {
      if authority != origin {
        tracing::warn!(
          authority = command.authority.authority(),
          origin_node = command.request.origin_node(),
          "remote deployment origin node differs from inbound authority; replying to inbound authority"
        );
      }
      Some(authority)
    },
    | (Some(authority), None) => Some(authority),
    | (None, Some(origin)) => {
      tracing::warn!(
        authority = command.authority.authority(),
        origin_node = command.request.origin_node(),
        "remote deployment inbound authority is invalid; falling back to origin node"
      );
      Some(origin)
    },
    | (None, None) => None,
  }
}
