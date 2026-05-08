//! Actor system extension installer for `remote-core`'s `Remote`.

use std::{
  sync::{Arc, Mutex, OnceLock},
  time::Instant,
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    actor_path::ActorPath, actor_ref::dead_letter::DeadLetterReason, extension::ExtensionInstaller,
    messaging::AnyMessage,
  },
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::core::{
  config::RemoteConfig,
  envelope::InboundEnvelope,
  extension::{EventPublisher, Remote, RemoteEvent, RemoteEventReceiver, RemoteShared, Remoting, RemotingError},
  instrument::RemotingFlightRecorder,
};
use futures::future::poll_fn;
use tokio::{
  runtime::Handle,
  sync::mpsc::{self, Sender},
  task::JoinHandle,
};

use crate::std::{tokio_remote_event_receiver::TokioMpscRemoteEventReceiver, transport::tcp::TcpRemoteTransport};

const ALREADY_INSTALLED: &str = "remote extension is already installed";
const TRANSPORT_LOCK_POISONED: &str = "remote extension transport lock is poisoned";
const RUN_STATE_LOCK_POISONED: &str = "remote run_state lock should not be poisoned";

/// Extension installer for the `fraktor-remote-adaptor-std-rs` runtime.
pub struct RemotingExtensionInstaller {
  transport:       Mutex<Option<TcpRemoteTransport>>,
  config:          RemoteConfig,
  remote_shared:   OnceLock<RemoteShared>,
  event_sender:    OnceLock<Sender<RemoteEvent>>,
  monotonic_epoch: OnceLock<Instant>,
  run_state:       Arc<Mutex<RemotingRunState>>,
}

struct RemotingRunState {
  receiver:           Option<TokioMpscRemoteEventReceiver>,
  handle:             Option<JoinHandle<(TokioMpscRemoteEventReceiver, Result<(), RemotingError>)>>,
  termination_handle: Option<JoinHandle<()>>,
}

impl RemotingRunState {
  const fn new() -> Self {
    Self { receiver: None, handle: None, termination_handle: None }
  }
}

impl Drop for RemotingRunState {
  fn drop(&mut self) {
    if let Some(handle) = self.handle.take() {
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
      monotonic_epoch: OnceLock::new(),
      run_state: Arc::new(Mutex::new(RemotingRunState::new())),
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
    let (event_sender, event_receiver) = mpsc::channel(self.config.remote_event_queue_size());
    let monotonic_epoch = Instant::now();
    let transport = transport.with_monotonic_epoch(monotonic_epoch).with_remote_event_sender(event_sender.clone());
    let event_publisher = EventPublisher::new(system.downgrade());
    let remote = RemoteShared::new(Remote::with_instrument(
      transport,
      self.config.clone(),
      event_publisher,
      Box::new(RemotingFlightRecorder::new(self.config.flight_recorder_capacity())),
    ));
    remote.start().map_err(remoting_build_error)?;
    let mut run_state =
      self.run_state.lock().map_err(|_| ActorSystemBuildError::Configuration(String::from(RUN_STATE_LOCK_POISONED)))?;
    run_state.receiver = Some(TokioMpscRemoteEventReceiver::new(event_receiver));
    spawn_run_task_with_state(&mut run_state, remote.clone(), system.clone()).map_err(remoting_build_error)?;
    spawn_shutdown_on_system_termination(
      system,
      remote.clone(),
      event_sender.clone(),
      &mut run_state,
      self.run_state.clone(),
    )
    .map_err(remoting_build_error)?;
    self
      .event_sender
      .set(event_sender)
      .map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))?;
    self
      .monotonic_epoch
      .set(monotonic_epoch)
      .map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))?;
    // ExtensionInstaller::install は &self 契約のため、一回限りの初期化に OnceLock を使う。
    self.remote_shared.set(remote).map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))
  }
}

fn remoting_build_error(error: RemotingError) -> ActorSystemBuildError {
  ActorSystemBuildError::Configuration(error.to_string())
}

fn spawn_run_task_with_state(
  run_state: &mut RemotingRunState,
  remote: RemoteShared,
  system: ActorSystem,
) -> Result<(), RemotingError> {
  if run_state.handle.is_some() {
    return Err(RemotingError::AlreadyRunning);
  }
  let Some(mut receiver) = run_state.receiver.take() else {
    unreachable!("spawn_run_task_with_state: receiver missing; install must set it before spawning");
  };
  let handle = Handle::try_current().map_err(|_| RemotingError::TransportUnavailable)?;
  let handle = handle.spawn(async move {
    let result = run_remote_with_delivery(&remote, &mut receiver, &system).await;
    (receiver, result)
  });
  run_state.handle = Some(handle);
  Ok(())
}

fn spawn_shutdown_on_system_termination(
  system: &ActorSystem,
  remote: RemoteShared,
  event_sender: Sender<RemoteEvent>,
  run_state: &mut RemotingRunState,
  run_state_shared: Arc<Mutex<RemotingRunState>>,
) -> Result<(), RemotingError> {
  let termination = system.when_terminated();
  let handle = Handle::try_current().map_err(|_| RemotingError::TransportUnavailable)?;
  let handle = handle.spawn(async move {
    termination.await;
    if let Err(error) = shutdown_remote_and_join(remote, Some(event_sender), run_state_shared).await {
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
) -> Result<(), RemotingError> {
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

async fn run_remote_with_delivery(
  remote: &RemoteShared,
  receiver: &mut TokioMpscRemoteEventReceiver,
  system: &ActorSystem,
) -> Result<(), RemotingError> {
  loop {
    let event = match poll_fn(|cx| receiver.poll_recv(cx)).await {
      | Some(event) => event,
      | None => return Err(RemotingError::EventReceiverClosed),
    };
    let should_stop = remote.handle_event(event)?;
    deliver_inbound_envelopes(remote, system);
    if should_stop {
      return Ok(());
    }
  }
}

fn deliver_inbound_envelopes(remote: &RemoteShared, system: &ActorSystem) {
  for envelope in remote.drain_inbound_envelopes() {
    deliver_inbound_envelope(envelope, system);
  }
}

pub(super) fn deliver_inbound_envelope(envelope: InboundEnvelope, system: &ActorSystem) {
  let (recipient, remote_node, message, sender, correlation_id, priority) = envelope.into_parts();
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
