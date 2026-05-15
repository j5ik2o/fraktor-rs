//! Std remote deployment hook.

use std::{
  net::{IpAddr, Ipv6Addr},
  string::{String, ToString},
  sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, RecvTimeoutError},
  },
  time::{Duration, Instant},
};

use bytes::Bytes;
use fraktor_actor_core_kernel_rs::{
  actor::{Address as ActorAddress, actor_path::ActorPathParser, props::DeployablePropsMetadata},
  serialization::{SerializationCallScope, SerializationExtensionShared, SerializedMessage},
  system::{
    ActorSystem,
    remote::{RemoteDeploymentHook, RemoteDeploymentOutcome, RemoteDeploymentRequest},
  },
};
use fraktor_remote_core_rs::{
  address::{Address, UniqueAddress},
  extension::RemoteEvent,
  wire::{RemoteDeploymentCreateRequest, RemoteDeploymentPdu},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};
use tokio::{
  runtime::{Handle, RuntimeFlavor},
  sync::mpsc::Sender,
};

use crate::{
  association::std_instant_elapsed_millis,
  deployment::{DeploymentResponse, DeploymentResponseDispatcher},
};

const DEPLOYABLE_METADATA_REQUIRED: &str = "remote deployment requires deployable props metadata";
const TARGET_HOST_REQUIRED: &str = "remote deployment target host is missing";
const TARGET_PORT_REQUIRED: &str = "remote deployment target port is missing";
const TARGET_PARENT_REQUIRED: &str = "remote deployment target parent path is missing";

/// Std adapter hook that turns actor-core remote deployment requests into wire create requests.
pub(crate) struct StdRemoteDeploymentHook {
  local_address:    UniqueAddress,
  system:           ActorSystem,
  event_sender:     Sender<RemoteEvent>,
  monotonic_epoch:  Instant,
  serialization:    ArcShared<SerializationExtensionShared>,
  dispatcher:       DeploymentResponseDispatcher,
  timeout:          Duration,
  next_correlation: AtomicU64,
}

impl StdRemoteDeploymentHook {
  /// Creates a std remote deployment hook.
  pub(crate) fn new(
    local_address: UniqueAddress,
    system: ActorSystem,
    event_sender: Sender<RemoteEvent>,
    monotonic_epoch: Instant,
    serialization: ArcShared<SerializationExtensionShared>,
    dispatcher: DeploymentResponseDispatcher,
    timeout: Duration,
  ) -> Self {
    Self {
      local_address,
      system,
      event_sender,
      monotonic_epoch,
      serialization,
      dispatcher,
      timeout,
      next_correlation: AtomicU64::new(1),
    }
  }
}

impl RemoteDeploymentHook for StdRemoteDeploymentHook {
  fn deploy_child(&self, request: RemoteDeploymentRequest) -> RemoteDeploymentOutcome {
    let target = match remote_target_address(request.scope().node(), &self.local_address) {
      | Ok(RemoteTarget::Local) => return RemoteDeploymentOutcome::UseLocalDeployment,
      | Ok(RemoteTarget::Remote(target)) => target,
      | Err(reason) => return RemoteDeploymentOutcome::Failed(reason),
    };
    let Some(metadata) = request.deployable_metadata() else {
      return RemoteDeploymentOutcome::Failed(String::from(DEPLOYABLE_METADATA_REQUIRED));
    };
    let create_request = match self.create_request(&request, metadata, &target) {
      | Ok(create_request) => create_request,
      | Err(reason) => return RemoteDeploymentOutcome::Failed(reason),
    };
    let correlation_hi = create_request.correlation_hi();
    let correlation_lo = create_request.correlation_lo();
    let receiver = self.dispatcher.register(correlation_hi, correlation_lo);
    let now_ms = std_instant_elapsed_millis(self.monotonic_epoch);
    if let Err(error) = self.event_sender.try_send(RemoteEvent::OutboundDeployment {
      remote: target,
      pdu: RemoteDeploymentPdu::CreateRequest(create_request),
      now_ms,
    }) {
      self.dispatcher.cancel(correlation_hi, correlation_lo);
      return RemoteDeploymentOutcome::Failed(format!("remote deployment request enqueue failed: {error:?}"));
    }
    match recv_deployment_response(&receiver, self.timeout) {
      | Ok(DeploymentResponse::Success(success)) => self.resolve_remote_actor(success.actor_path()),
      | Ok(DeploymentResponse::Failure(failure)) => {
        RemoteDeploymentOutcome::Failed(format!("{:?}: {}", failure.code(), failure.reason()))
      },
      | Err(_) => {
        self.dispatcher.cancel(correlation_hi, correlation_lo);
        RemoteDeploymentOutcome::Failed(String::from("remote deployment timed out"))
      },
    }
  }
}

fn recv_deployment_response(
  receiver: &Receiver<DeploymentResponse>,
  timeout: Duration,
) -> Result<DeploymentResponse, RecvTimeoutError> {
  match Handle::try_current() {
    | Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::MultiThread => {
      tokio::task::block_in_place(|| receiver.recv_timeout(timeout))
    },
    | _ => receiver.recv_timeout(timeout),
  }
}

impl StdRemoteDeploymentHook {
  fn create_request(
    &self,
    request: &RemoteDeploymentRequest,
    metadata: &DeployablePropsMetadata,
    target: &Address,
  ) -> Result<RemoteDeploymentCreateRequest, String> {
    let serialized = self.serialize_payload(metadata)?;
    let target_parent_path = target_parent_path(request, target)?;
    let correlation_hi = self.next_correlation.fetch_add(1, Ordering::Relaxed);
    Ok(RemoteDeploymentCreateRequest::new(
      correlation_hi,
      0,
      target_parent_path,
      request.child_name().to_string(),
      metadata.factory_id().to_string(),
      self.local_address.address().to_string(),
      serialized.serializer_id().value(),
      serialized.manifest().map(ToString::to_string),
      Bytes::copy_from_slice(serialized.bytes()),
    ))
  }

  fn serialize_payload(&self, metadata: &DeployablePropsMetadata) -> Result<SerializedMessage, String> {
    self
      .serialization
      .with_read(|serialization| serialization.serialize(metadata.payload().payload(), SerializationCallScope::Remote))
      .map_err(|error| format!("remote deployment payload serialization failed: {error:?}"))
  }

  fn resolve_remote_actor(&self, actor_path: &str) -> RemoteDeploymentOutcome {
    let path = match ActorPathParser::parse(actor_path) {
      | Ok(path) => path,
      | Err(error) => {
        return RemoteDeploymentOutcome::Failed(format!("remote deployment actor path is invalid: {error:?}"));
      },
    };
    match self.system.resolve_actor_ref(path) {
      | Ok(actor_ref) => RemoteDeploymentOutcome::RemoteCreated(actor_ref),
      | Err(error) => {
        RemoteDeploymentOutcome::Failed(format!("remote deployment actor ref resolution failed: {error:?}"))
      },
    }
  }
}

enum RemoteTarget {
  Local,
  Remote(Address),
}

fn remote_target_address(scope_node: &ActorAddress, local_address: &UniqueAddress) -> Result<RemoteTarget, String> {
  let host = scope_node.host().ok_or_else(|| String::from(TARGET_HOST_REQUIRED))?;
  let port = scope_node.port().ok_or_else(|| String::from(TARGET_PORT_REQUIRED))?;
  let target = Address::new(scope_node.system(), host, port);
  if local_address.address() == &target {
    return Ok(RemoteTarget::Local);
  }
  Ok(RemoteTarget::Remote(target))
}

fn target_parent_path(request: &RemoteDeploymentRequest, target: &Address) -> Result<String, String> {
  let segments = request.child_path().segments();
  let parent_len = segments.len().checked_sub(1).ok_or_else(|| String::from(TARGET_PARENT_REQUIRED))?;
  if parent_len == 0 {
    return Err(String::from(TARGET_PARENT_REQUIRED));
  }
  let mut path = format!("fraktor.tcp://{}@{}:{}", target.system(), uri_host(target.host()), target.port());
  for segment in segments.iter().take(parent_len) {
    path.push('/');
    path.push_str(segment.as_str());
  }
  Ok(path)
}

fn uri_host(host: &str) -> String {
  if host.starts_with('[') {
    return host.to_string();
  }
  match host.parse::<IpAddr>() {
    | Ok(IpAddr::V6(_)) => format!("[{host}]"),
    | Ok(IpAddr::V4(_)) => host.to_string(),
    | Err(_) if host.parse::<Ipv6Addr>().is_ok() || host.contains(':') => format!("[{host}]"),
    | Err(_) => host.to_string(),
  }
}
