//! Actor system extension installer for `remote-core`'s `Remote`.

#[cfg(test)]
#[path = "remoting_extension_installer_test.rs"]
mod tests;

use std::{
  sync::{Arc, Mutex, OnceLock},
  time::{Duration, Instant},
};

use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::ActorPath,
    actor_ref::dead_letter::DeadLetterReason,
    extension::ExtensionInstaller,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  event::stream::CorrelationId,
  serialization::{SerializationExtensionShared, default_serialization_extension_id},
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId},
  config::RemoteConfig,
  envelope::{InboundEnvelope, OutboundPriority},
  extension::{EventPublisher, Remote, RemoteEvent, RemoteEventReceiver, RemoteShared, Remoting, RemotingError},
  instrument::RemotingFlightRecorder,
  transport::RemoteTransport,
  watcher::WatcherCommand,
  wire::{
    ControlPdu, FlushScope, RemoteDeploymentCreateFailure, RemoteDeploymentCreateRequest, RemoteDeploymentFailureCode,
    RemoteDeploymentPdu, WireFrame,
  },
};
use fraktor_utils_core_rs::sync::ArcShared;
use futures::future::poll_fn;
use tokio::{
  runtime::Handle,
  sync::mpsc::{self, Receiver, Sender},
  task::JoinHandle,
};

use crate::{
  association::std_instant_elapsed_millis,
  deployment::{DeploymentDaemonCommand, DeploymentResponse, DeploymentResponseDispatcher, spawn_deployment_daemon},
  extension_installer::flush_gate::{StdFlushGate, schedule_flush_timers},
  tokio_remote_event_receiver::TokioMpscRemoteEventReceiver,
  transport::tcp::TcpRemoteTransport,
  watcher::run_watcher_task,
};

const ALREADY_INSTALLED: &str = "remote extension is already installed";
const TRANSPORT_LOCK_POISONED: &str = "remote extension transport lock is poisoned";
const RUN_STATE_LOCK_POISONED: &str = "remote run_state lock should not be poisoned";

/// Extension installer for the `fraktor-remote-adaptor-std-rs` runtime.
pub struct RemotingExtensionInstaller {
  transport: Mutex<Option<TcpRemoteTransport>>,
  config: RemoteConfig,
  remote_shared: OnceLock<RemoteShared>,
  event_sender: OnceLock<Sender<RemoteEvent>>,
  watcher_sender: OnceLock<Sender<WatcherCommand>>,
  monotonic_epoch: OnceLock<Instant>,
  run_state: Arc<Mutex<RemotingRunState>>,
  flush_gate: StdFlushGate,
  deployment_response_dispatcher: DeploymentResponseDispatcher,
}

pub(super) struct RemotingRunState {
  receiver:           Option<TokioMpscRemoteEventReceiver>,
  handle:             Option<JoinHandle<(TokioMpscRemoteEventReceiver, Result<(), RemotingError>)>>,
  watcher_handle:     Option<JoinHandle<()>>,
  deployment_handle:  Option<JoinHandle<()>>,
  termination_handle: Option<JoinHandle<()>>,
}

/// Handles needed by the std remote actor-ref provider.
pub(crate) struct RemoteProviderFlushHandles {
  /// Remote event sender.
  pub(crate) event_sender:                   Sender<RemoteEvent>,
  /// Monotonic epoch shared with remoting tasks.
  pub(crate) monotonic_epoch:                Instant,
  /// Watcher command sender.
  pub(crate) watcher_sender:                 Sender<WatcherCommand>,
  /// Shared remote core handle.
  pub(crate) remote_shared:                  RemoteShared,
  /// Shared std flush gate.
  pub(crate) flush_gate:                     StdFlushGate,
  /// Message-capable writer lane ids.
  pub(crate) flush_lane_ids:                 Vec<u32>,
  /// Deployment response dispatcher.
  pub(crate) deployment_response_dispatcher: DeploymentResponseDispatcher,
  /// Deployment request timeout.
  pub(crate) deployment_timeout:             Duration,
}

#[derive(Clone)]
struct ShutdownFlushContext {
  config:          RemoteConfig,
  monotonic_epoch: Instant,
  flush_gate:      StdFlushGate,
}

impl RemotingRunState {
  pub(super) const fn new() -> Self {
    Self {
      receiver:           None,
      handle:             None,
      watcher_handle:     None,
      deployment_handle:  None,
      termination_handle: None,
    }
  }
}

impl Drop for RemotingRunState {
  fn drop(&mut self) {
    if let Some(handle) = self.handle.take() {
      handle.abort();
    }
    if let Some(handle) = self.watcher_handle.take() {
      handle.abort();
    }
    if let Some(handle) = self.deployment_handle.take() {
      handle.abort();
    }
    if let Some(handle) = self.termination_handle.take() {
      handle.abort();
    }
  }
}

impl RemotingExtensionInstaller {
  /// Creates a new installer that will move the given transport into
  /// `remote-core`'s [`Remote`] during installation.
  #[must_use]
  pub fn new(transport: TcpRemoteTransport, config: RemoteConfig) -> Self {
    Self {
      transport: Mutex::new(Some(transport)),
      config,
      remote_shared: OnceLock::new(),
      event_sender: OnceLock::new(),
      watcher_sender: OnceLock::new(),
      monotonic_epoch: OnceLock::new(),
      run_state: Arc::new(Mutex::new(RemotingRunState::new())),
      flush_gate: StdFlushGate::new(),
      deployment_response_dispatcher: DeploymentResponseDispatcher::default(),
    }
  }

  /// Returns the adapter event sender and monotonic epoch created during install.
  ///
  /// This is used by companion actor-ref provider installers that are also
  /// installed during `ActorSystemConfig` bootstrap and must enqueue into the
  /// same remote event loop.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] when the remote extension has not
  /// been installed yet.
  pub(crate) fn remote_event_sender_and_epoch(&self) -> Result<(Sender<RemoteEvent>, Instant), RemotingError> {
    let sender = self.event_sender.get().cloned().ok_or(RemotingError::NotStarted)?;
    let monotonic_epoch = self.monotonic_epoch.get().copied().ok_or(RemotingError::NotStarted)?;
    Ok((sender, monotonic_epoch))
  }

  pub(crate) fn remote_event_sender_epoch_and_watcher(
    &self,
  ) -> Result<(Sender<RemoteEvent>, Instant, Sender<WatcherCommand>), RemotingError> {
    let (sender, monotonic_epoch) = self.remote_event_sender_and_epoch()?;
    let watcher_sender = self.watcher_sender.get().cloned().ok_or(RemotingError::NotStarted)?;
    Ok((sender, monotonic_epoch, watcher_sender))
  }

  pub(crate) fn remote_event_sender_epoch_watcher_and_flush(
    &self,
  ) -> Result<RemoteProviderFlushHandles, RemotingError> {
    let (sender, monotonic_epoch, watcher_sender) = self.remote_event_sender_epoch_and_watcher()?;
    let remote_shared = self.remote_shared.get().cloned().ok_or(RemotingError::NotStarted)?;
    Ok(RemoteProviderFlushHandles {
      event_sender: sender,
      monotonic_epoch,
      watcher_sender,
      remote_shared,
      flush_gate: self.flush_gate.clone(),
      flush_lane_ids: writer_lane_ids_for_config(&self.config),
      deployment_response_dispatcher: self.deployment_response_dispatcher.clone(),
      deployment_timeout: self.config.deployment_timeout(),
    })
  }

  /// Runs graceful remote shutdown and joins the run task.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError`] when the remote was not installed, shutdown
  /// fails, or the run task returns an error.
  pub async fn shutdown_and_join(&self) -> Result<(), RemotingError> {
    let remote = self.remote_shared.get().cloned().ok_or(RemotingError::NotStarted)?;
    let event_sender = self.event_sender.get().cloned();
    let monotonic_epoch = self.monotonic_epoch.get().copied().ok_or(RemotingError::NotStarted)?;
    shutdown_remote_and_join(
      remote,
      event_sender,
      self.run_state.clone(),
      self.config.clone(),
      monotonic_epoch,
      self.flush_gate.clone(),
    )
    .await
  }
}

impl ExtensionInstaller for RemotingExtensionInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let mut transport_slot =
      self.transport.lock().map_err(|_| ActorSystemBuildError::Configuration(String::from(TRANSPORT_LOCK_POISONED)))?;
    if self.remote_shared.get().is_some() {
      return Err(ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)));
    }
    let Some(transport) = transport_slot.take() else {
      return Err(ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)));
    };
    let serialization_extension = system.extended().register_extension(&default_serialization_extension_id());
    let (event_sender, event_receiver) = mpsc::channel(self.config.remote_event_queue_size());
    let (watcher_sender, watcher_receiver) = mpsc::channel(self.config.remote_event_queue_size());
    let (deployment_sender, deployment_receiver) = mpsc::channel(self.config.remote_event_queue_size());
    let monotonic_epoch = Instant::now();
    let transport = transport
      .with_monotonic_epoch(monotonic_epoch)
      .with_remote_event_sender(event_sender.clone())
      .with_serialization_extension(serialization_extension.clone());
    let local_address =
      transport.default_address().or_else(|| transport.addresses().first()).cloned().unwrap_or_else(|| {
        Address::new(
          system.state().system_name(),
          self.config.canonical_host(),
          self.config.canonical_port().or_else(|| self.config.bind_port()).unwrap_or(0),
        )
      });
    let event_publisher = EventPublisher::new(system.downgrade());
    let remote = RemoteShared::new(Remote::with_instrument(
      transport,
      self.config.clone(),
      event_publisher,
      serialization_extension.clone(),
      Box::new(RemotingFlightRecorder::new(self.config.flight_recorder_capacity())),
    ));
    remote.start().map_err(remoting_build_error)?;
    let install_result = (|| -> Result<(), ActorSystemBuildError> {
      let mut run_state = self
        .run_state
        .lock()
        .map_err(|_| ActorSystemBuildError::Configuration(String::from(RUN_STATE_LOCK_POISONED)))?;
      run_state.receiver = Some(TokioMpscRemoteEventReceiver::new(event_receiver));
      spawn_run_task_with_state(
        &mut run_state,
        remote.clone(),
        system.clone(),
        watcher_sender.clone(),
        deployment_sender.clone(),
        self.deployment_response_dispatcher.clone(),
        event_sender.clone(),
        self.flush_gate.clone(),
      )
      .map_err(remoting_build_error)?;
      spawn_watcher_task_with_state(
        &mut run_state,
        watcher_receiver,
        event_sender.clone(),
        system.clone(),
        local_address,
        monotonic_epoch,
        self.config.system_message_resend_interval(),
      )
      .map_err(remoting_build_error)?;
      spawn_deployment_daemon_with_state(
        &mut run_state,
        deployment_receiver,
        system.clone(),
        serialization_extension.clone(),
        event_sender.clone(),
        monotonic_epoch,
      )
      .map_err(remoting_build_error)?;
      let shutdown_flush_context =
        ShutdownFlushContext { config: self.config.clone(), monotonic_epoch, flush_gate: self.flush_gate.clone() };
      spawn_shutdown_on_system_termination(
        system,
        remote.clone(),
        event_sender.clone(),
        &mut run_state,
        self.run_state.clone(),
        shutdown_flush_context,
      )
      .map_err(remoting_build_error)?;
      self
        .event_sender
        .set(event_sender.clone())
        .map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))?;
      self
        .watcher_sender
        .set(watcher_sender.clone())
        .map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))?;
      self
        .monotonic_epoch
        .set(monotonic_epoch)
        .map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))?;
      // ExtensionInstaller::install は &self 契約のため、一回限りの初期化に OnceLock を使う。
      self
        .remote_shared
        .set(remote.clone())
        .map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))?;
      Ok(())
    })();
    if let Err(error) = install_result {
      rollback_started_remote(&remote, &event_sender, &self.run_state);
      return Err(error);
    }
    Ok(())
  }
}

fn remoting_build_error(error: RemotingError) -> ActorSystemBuildError {
  ActorSystemBuildError::Configuration(error.to_string())
}

pub(super) fn rollback_started_remote(
  remote: &RemoteShared,
  event_sender: &Sender<RemoteEvent>,
  run_state: &Arc<Mutex<RemotingRunState>>,
) {
  if let Err(error) = remote.shutdown() {
    tracing::debug!(?error, "remote install rollback shutdown failed");
  }
  if let Err(error) = event_sender.try_send(RemoteEvent::TransportShutdown) {
    tracing::debug!(?error, "remote install rollback wake failed");
  }
  match run_state.lock() {
    | Ok(mut run_state) => {
      if let Some(handle) = run_state.handle.take() {
        handle.abort();
      }
      if let Some(handle) = run_state.watcher_handle.take() {
        handle.abort();
      }
      if let Some(handle) = run_state.deployment_handle.take() {
        handle.abort();
      }
      if let Some(handle) = run_state.termination_handle.take() {
        handle.abort();
      }
    },
    | Err(_) => tracing::debug!("remote install rollback run_state lock failed"),
  }
}

#[allow(clippy::too_many_arguments)]
fn spawn_run_task_with_state(
  run_state: &mut RemotingRunState,
  remote: RemoteShared,
  system: ActorSystem,
  watcher_sender: Sender<WatcherCommand>,
  deployment_sender: Sender<DeploymentDaemonCommand>,
  deployment_response_dispatcher: DeploymentResponseDispatcher,
  event_sender: Sender<RemoteEvent>,
  flush_gate: StdFlushGate,
) -> Result<(), RemotingError> {
  if run_state.handle.is_some() {
    return Err(RemotingError::AlreadyRunning);
  }
  let Some(mut receiver) = run_state.receiver.take() else {
    unreachable!("spawn_run_task_with_state: receiver missing; install must set it before spawning");
  };
  let handle = Handle::try_current().map_err(|_| RemotingError::TransportUnavailable)?;
  let handle = handle.spawn(async move {
    let result = run_remote_with_delivery(
      &remote,
      &mut receiver,
      &system,
      &watcher_sender,
      &deployment_sender,
      deployment_response_dispatcher,
      &event_sender,
      &flush_gate,
    )
    .await;
    (receiver, result)
  });
  run_state.handle = Some(handle);
  Ok(())
}

fn spawn_deployment_daemon_with_state(
  run_state: &mut RemotingRunState,
  deployment_receiver: Receiver<DeploymentDaemonCommand>,
  system: ActorSystem,
  serialization_extension: ArcShared<SerializationExtensionShared>,
  event_sender: Sender<RemoteEvent>,
  monotonic_epoch: Instant,
) -> Result<(), RemotingError> {
  if run_state.deployment_handle.is_some() {
    return Err(RemotingError::AlreadyRunning);
  }
  run_state.deployment_handle =
    Some(spawn_deployment_daemon(deployment_receiver, system, serialization_extension, event_sender, monotonic_epoch));
  Ok(())
}

pub(super) fn spawn_watcher_task_with_state(
  run_state: &mut RemotingRunState,
  watcher_receiver: Receiver<WatcherCommand>,
  event_sender: Sender<RemoteEvent>,
  system: ActorSystem,
  local_address: Address,
  monotonic_epoch: Instant,
  tick_interval: Duration,
) -> Result<(), RemotingError> {
  if run_state.watcher_handle.is_some() {
    return Err(RemotingError::AlreadyRunning);
  }
  let handle = Handle::try_current().map_err(|_| RemotingError::TransportUnavailable)?;
  let handle = handle.spawn(run_watcher_task(
    watcher_receiver,
    event_sender,
    system,
    local_address,
    monotonic_epoch,
    tick_interval,
  ));
  run_state.watcher_handle = Some(handle);
  Ok(())
}

fn spawn_shutdown_on_system_termination(
  system: &ActorSystem,
  remote: RemoteShared,
  event_sender: Sender<RemoteEvent>,
  run_state: &mut RemotingRunState,
  run_state_shared: Arc<Mutex<RemotingRunState>>,
  flush_context: ShutdownFlushContext,
) -> Result<(), RemotingError> {
  let termination = system.when_terminated();
  let handle = Handle::try_current().map_err(|_| RemotingError::TransportUnavailable)?;
  let handle = handle.spawn(async move {
    termination.await;
    if let Err(error) = shutdown_remote_and_join(
      remote,
      Some(event_sender),
      run_state_shared,
      flush_context.config,
      flush_context.monotonic_epoch,
      flush_context.flush_gate,
    )
    .await
    {
      tracing::debug!(?error, "remote termination shutdown task failed");
    }
  });
  if let Some(previous) = run_state.termination_handle.replace(handle) {
    previous.abort();
  }
  Ok(())
}

async fn shutdown_remote_and_join(
  remote: RemoteShared,
  event_sender: Option<Sender<RemoteEvent>>,
  run_state: Arc<Mutex<RemotingRunState>>,
  config: RemoteConfig,
  monotonic_epoch: Instant,
  flush_gate: StdFlushGate,
) -> Result<(), RemotingError> {
  if let Some(sender) = event_sender.as_ref()
    && let Err(error) = wait_for_shutdown_flush(&remote, sender, &config, monotonic_epoch, &flush_gate).await
  {
    tracing::debug!(?error, "remote shutdown flush failed; continuing shutdown");
  }
  let shutdown_result = remote.shutdown();
  if let Err(error) = &shutdown_result {
    tracing::debug!(?error, "remote shutdown failed; still attempting to join run task");
  }
  if let Some(sender) = event_sender {
    // ベストエフォートの wake。Full は pending event が shutdown を観測できる状態、
    // Closed は receiver が既に終了しており join 側がそれを観測する状態として扱う。
    if let Err(send_err) = sender.try_send(RemoteEvent::TransportShutdown) {
      tracing::debug!(?send_err, "shutdown wake failed");
    }
  }
  let handle = {
    let mut run_state = run_state.lock().expect(RUN_STATE_LOCK_POISONED);
    if let Some(handle) = run_state.watcher_handle.take() {
      handle.abort();
    }
    if let Some(handle) = run_state.deployment_handle.take() {
      handle.abort();
    }
    run_state.handle.take()
  };
  let Some(handle) = handle else {
    return shutdown_result;
  };
  let join_result = match join_run_handle(handle).await {
    | Ok((receiver, result)) => {
      let mut run_state = run_state.lock().expect(RUN_STATE_LOCK_POISONED);
      run_state.receiver = Some(receiver);
      result
    },
    | Err(error) => Err(error),
  };
  match (shutdown_result, join_result) {
    | (Ok(()), Ok(())) => Ok(()),
    | (Ok(()), Err(error)) => Err(error),
    | (Err(error), Ok(())) => Err(error),
    | (Err(shutdown_error), Err(join_error)) => {
      tracing::warn!(?shutdown_error, ?join_error, "remote shutdown failed before run task join failed");
      Err(join_error)
    },
  }
}

async fn wait_for_shutdown_flush(
  remote: &RemoteShared,
  event_sender: &Sender<RemoteEvent>,
  config: &RemoteConfig,
  monotonic_epoch: Instant,
  flush_gate: &StdFlushGate,
) -> Result<(), RemotingError> {
  let lane_ids = writer_lane_ids_for_config(config);
  let now_ms = std_instant_elapsed_millis(monotonic_epoch);
  let (timers, outcomes) = remote.start_flush_and_drain_outcomes(None, FlushScope::Shutdown, &lane_ids, now_ms)?;
  let waiter = flush_gate.register_shutdown_waiter(&timers);
  schedule_flush_timers(event_sender, monotonic_epoch, &timers);
  flush_gate.observe_outcomes(outcomes, event_sender);
  if let Some(waiter) = waiter
    && !waiter.wait(config.shutdown_flush_timeout()).await
  {
    tracing::warn!("remote shutdown flush timed out");
  }
  Ok(())
}

fn writer_lane_ids_for_config(config: &RemoteConfig) -> Vec<u32> {
  (0..config.outbound_lanes()).filter_map(|lane_id| u32::try_from(lane_id).ok()).collect()
}

#[allow(clippy::too_many_arguments)]
async fn run_remote_with_delivery(
  remote: &RemoteShared,
  receiver: &mut TokioMpscRemoteEventReceiver,
  system: &ActorSystem,
  watcher_sender: &Sender<WatcherCommand>,
  deployment_sender: &Sender<DeploymentDaemonCommand>,
  deployment_response_dispatcher: DeploymentResponseDispatcher,
  event_sender: &Sender<RemoteEvent>,
  flush_gate: &StdFlushGate,
) -> Result<(), RemotingError> {
  loop {
    if remote.should_stop_event_loop() {
      return Ok(());
    }
    let event = match poll_fn(|cx| receiver.poll_recv(cx)).await {
      | Some(event) => event,
      | None => return Err(RemotingError::EventReceiverClosed),
    };
    let Some(event) = route_deployment_event(event, deployment_sender, &deployment_response_dispatcher) else {
      continue;
    };
    forward_watcher_command_for_event(&event, watcher_sender);
    let should_stop = remote.handle_event(event)?;
    flush_gate.observe_outcomes(remote.drain_flush_outcomes(), event_sender);
    deliver_inbound_envelopes(remote, system);
    if should_stop {
      return Ok(());
    }
  }
}

fn route_deployment_event(
  event: RemoteEvent,
  deployment_sender: &Sender<DeploymentDaemonCommand>,
  deployment_response_dispatcher: &DeploymentResponseDispatcher,
) -> Option<RemoteEvent> {
  let RemoteEvent::InboundFrameReceived { authority, frame: WireFrame::Deployment(pdu), now_ms } = event else {
    return Some(event);
  };
  match pdu {
    | RemoteDeploymentPdu::CreateRequest(request) => {
      let command = DeploymentDaemonCommand::create(authority, request);
      if let Err(error) = deployment_sender.try_send(command) {
        let reason = error.to_string();
        let command = error.into_inner();
        tracing::warn!(?reason, "remote deployment request enqueue failed");
        let Some(remote) =
          parse_address(command.authority.authority()).or_else(|| parse_address(command.request.origin_node()))
        else {
          tracing::warn!(
            authority = command.authority.authority(),
            origin_node = command.request.origin_node(),
            "remote deployment request enqueue failure response address is invalid"
          );
          return None;
        };
        return Some(RemoteEvent::OutboundDeployment {
          remote,
          pdu: deployment_enqueue_failure(&command.request, reason),
          now_ms,
        });
      }
    },
    | RemoteDeploymentPdu::CreateSuccess(success) => {
      deployment_response_dispatcher.complete(DeploymentResponse::Success(success));
    },
    | RemoteDeploymentPdu::CreateFailure(failure) => {
      deployment_response_dispatcher.complete(DeploymentResponse::Failure(failure));
    },
  }
  None
}

fn deployment_enqueue_failure(request: &RemoteDeploymentCreateRequest, reason: String) -> RemoteDeploymentPdu {
  RemoteDeploymentPdu::CreateFailure(RemoteDeploymentCreateFailure::new(
    request.correlation_hi(),
    request.correlation_lo(),
    RemoteDeploymentFailureCode::SpawnFailed,
    reason,
  ))
}

pub(super) fn forward_watcher_command_for_event(event: &RemoteEvent, watcher_sender: &Sender<WatcherCommand>) {
  let RemoteEvent::InboundFrameReceived { frame, now_ms, .. } = event else {
    return;
  };
  let command = match frame {
    | WireFrame::Control(ControlPdu::Heartbeat { authority }) => {
      parse_address(authority).map(|from| WatcherCommand::HeartbeatReceived { from, now: *now_ms })
    },
    | WireFrame::Control(ControlPdu::HeartbeatResponse { authority, uid }) => {
      parse_address(authority).map(|from| WatcherCommand::HeartbeatResponseReceived { from, uid: *uid, now: *now_ms })
    },
    | WireFrame::Control(
      ControlPdu::Quarantine { .. }
      | ControlPdu::Shutdown { .. }
      | ControlPdu::FlushRequest { .. }
      | ControlPdu::FlushAck { .. }
      | ControlPdu::CompressionAdvertisement { .. }
      | ControlPdu::CompressionAck { .. },
    )
    | WireFrame::Envelope(_)
    | WireFrame::Handshake(_)
    | WireFrame::Ack(_)
    | WireFrame::Deployment(_) => None,
  };
  let Some(command) = command else {
    return;
  };
  if let Err(error) = watcher_sender.try_send(command) {
    tracing::warn!(?error, "remote watcher inbound command enqueue failed");
  }
}

fn parse_address(authority: &str) -> Option<Address> {
  let (system, endpoint) = authority.split_once('@')?;
  let (host, port) = endpoint.rsplit_once(':')?;
  let host = host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
  Some(Address::new(system, host, port.parse::<u16>().ok()?))
}

fn deliver_inbound_envelopes(remote: &RemoteShared, system: &ActorSystem) {
  for envelope in remote.drain_inbound_envelopes() {
    deliver_inbound_envelope(envelope, system);
  }
}

pub(super) fn deliver_inbound_envelope(envelope: InboundEnvelope, system: &ActorSystem) {
  let (recipient, remote_node, message, sender, correlation_id, priority) = envelope.into_parts();
  if priority.is_system()
    && let Some(system_message) = message.downcast_ref::<SystemMessage>().cloned()
  {
    deliver_inbound_system_envelope(recipient, remote_node, system_message, sender, correlation_id, priority, system);
    return;
  }
  let message = attach_sender(system, sender, message);
  let mut actor_ref = match system.resolve_actor_ref(recipient.clone()) {
    | Ok(actor_ref) => actor_ref,
    | Err(error) => {
      tracing::warn!(
        ?error,
        recipient = %recipient,
        ?remote_node,
        ?correlation_id,
        ?priority,
        "remote inbound delivery recipient resolution failed"
      );
      system.record_dead_letter(message, DeadLetterReason::RecipientUnavailable, None);
      return;
    },
  };
  if let Err(error) = actor_ref.try_tell(message) {
    tracing::warn!(
      ?error,
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound delivery send failed"
    );
  }
}

fn deliver_inbound_system_envelope(
  recipient: ActorPath,
  remote_node: RemoteNodeId,
  system_message: SystemMessage,
  sender: Option<ActorPath>,
  correlation_id: CorrelationId,
  priority: OutboundPriority,
  system: &ActorSystem,
) {
  match system_message {
    | SystemMessage::Watch(_) => {
      deliver_inbound_watch_message(
        recipient,
        remote_node,
        sender,
        correlation_id,
        priority,
        system,
        SystemMessage::Watch,
      );
    },
    | SystemMessage::Unwatch(_) => {
      deliver_inbound_watch_message(
        recipient,
        remote_node,
        sender,
        correlation_id,
        priority,
        system,
        SystemMessage::Unwatch,
      );
    },
    | SystemMessage::DeathWatchNotification(_) => {
      deliver_inbound_deathwatch_notification(recipient, remote_node, sender, correlation_id, priority, system);
    },
    | other => {
      deliver_inbound_system_message_to_recipient(recipient, remote_node, other, correlation_id, priority, system);
    },
  }
}

fn deliver_inbound_watch_message(
  recipient: ActorPath,
  remote_node: RemoteNodeId,
  sender: Option<ActorPath>,
  correlation_id: CorrelationId,
  priority: OutboundPriority,
  system: &ActorSystem,
  make_message: fn(Pid) -> SystemMessage,
) {
  let Some(sender) = sender else {
    tracing::warn!(
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound system delivery missing sender path"
    );
    system.record_dead_letter(AnyMessage::new(make_message(Pid::new(0, 0))), DeadLetterReason::MissingRecipient, None);
    return;
  };
  let Some(target_pid) = resolve_pid(system, &recipient, DeadLetterReason::RecipientUnavailable) else {
    tracing::warn!(
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound system delivery recipient resolution failed"
    );
    return;
  };
  let Some(watcher_pid) = resolve_pid(system, &sender, DeadLetterReason::MissingRecipient) else {
    tracing::warn!(
      sender = %sender,
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound system delivery sender resolution failed"
    );
    return;
  };
  if let Err(error) = system.state().send_system_message(target_pid, make_message(watcher_pid)) {
    tracing::warn!(
      ?error,
      recipient = %recipient,
      sender = %sender,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound system delivery failed"
    );
    system.state().record_send_error(Some(target_pid), &error);
  }
}

fn deliver_inbound_deathwatch_notification(
  recipient: ActorPath,
  remote_node: RemoteNodeId,
  sender: Option<ActorPath>,
  correlation_id: CorrelationId,
  priority: OutboundPriority,
  system: &ActorSystem,
) {
  let Some(sender) = sender else {
    tracing::warn!(
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound death-watch notification missing sender path"
    );
    system.record_dead_letter(
      AnyMessage::new(SystemMessage::DeathWatchNotification(Pid::new(0, 0))),
      DeadLetterReason::MissingRecipient,
      None,
    );
    return;
  };
  let Some(watcher_pid) = resolve_pid(system, &recipient, DeadLetterReason::RecipientUnavailable) else {
    tracing::warn!(
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound death-watch watcher resolution failed"
    );
    return;
  };
  let Some(target_pid) = resolve_pid(system, &sender, DeadLetterReason::MissingRecipient) else {
    tracing::warn!(
      sender = %sender,
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound death-watch target resolution failed"
    );
    return;
  };
  if let Err(error) = system.state().send_system_message(watcher_pid, SystemMessage::DeathWatchNotification(target_pid))
  {
    tracing::warn!(
      ?error,
      recipient = %recipient,
      sender = %sender,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound death-watch notification delivery failed"
    );
    system.state().record_send_error(Some(watcher_pid), &error);
  }
}

fn deliver_inbound_system_message_to_recipient(
  recipient: ActorPath,
  remote_node: RemoteNodeId,
  system_message: SystemMessage,
  correlation_id: CorrelationId,
  priority: OutboundPriority,
  system: &ActorSystem,
) {
  let Some(recipient_pid) = resolve_pid(system, &recipient, DeadLetterReason::RecipientUnavailable) else {
    tracing::warn!(
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound system message recipient resolution failed"
    );
    system.record_dead_letter(AnyMessage::new(system_message), DeadLetterReason::RecipientUnavailable, None);
    return;
  };
  if let Err(error) = system.state().send_system_message(recipient_pid, system_message) {
    tracing::warn!(
      ?error,
      recipient = %recipient,
      ?remote_node,
      ?correlation_id,
      ?priority,
      "remote inbound system message delivery failed"
    );
    system.state().record_send_error(Some(recipient_pid), &error);
  }
}

fn resolve_pid(system: &ActorSystem, path: &ActorPath, dead_letter_reason: DeadLetterReason) -> Option<Pid> {
  if let Some(pid) = system.pid_by_path(path) {
    return Some(pid);
  }
  match system.resolve_actor_ref(path.clone()) {
    | Ok(actor_ref) => Some(actor_ref.pid()),
    | Err(error) => {
      tracing::debug!(?error, path = %path, "remote inbound actor path resolution failed");
      system.record_dead_letter(AnyMessage::new(path.clone()), dead_letter_reason, None);
      None
    },
  }
}

fn attach_sender(system: &ActorSystem, sender: Option<ActorPath>, message: AnyMessage) -> AnyMessage {
  let Some(sender_path) = sender else {
    return message;
  };
  match system.resolve_actor_ref(sender_path.clone()) {
    | Ok(sender_ref) => message.with_sender(sender_ref),
    | Err(error) => {
      tracing::debug!(?error, sender = %sender_path, "remote inbound sender restoration failed");
      message
    },
  }
}

async fn join_run_handle(
  handle: JoinHandle<(TokioMpscRemoteEventReceiver, Result<(), RemotingError>)>,
) -> Result<(TokioMpscRemoteEventReceiver, Result<(), RemotingError>), RemotingError> {
  match handle.await {
    | Ok(result) => Ok(result),
    | Err(join_error) => {
      tracing::error!(?join_error, "remote run task join failed");
      Err(RemotingError::TransportUnavailable)
    },
  }
}
