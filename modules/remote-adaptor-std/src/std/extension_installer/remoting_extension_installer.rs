//! Actor system extension installer for `remote-core`'s `Remote`.

use std::{
  sync::{Mutex, OnceLock},
  time::Instant,
};

use fraktor_actor_core_rs::core::kernel::{
  actor::extension::ExtensionInstaller,
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::core::{
  config::RemoteConfig,
  extension::{EventPublisher, Remote, RemoteEvent, RemoteShared, Remoting, RemotingError},
  instrument::RemotingFlightRecorder,
};
use tokio::{
  sync::mpsc::{self, Sender},
  task::JoinHandle,
};

use crate::std::{TokioMpscRemoteEventReceiver, transport::tcp::TcpRemoteTransport};

const ALREADY_INSTALLED: &str = "remote extension is already installed";
const TRANSPORT_LOCK_POISONED: &str = "remote extension transport lock is poisoned";
const RUN_STATE_LOCK_POISONED: &str = "remote run_state lock should not be poisoned";

/// Extension installer for the `fraktor-remote-adaptor-std-rs` runtime.
pub struct RemotingExtensionInstaller {
  transport:     Mutex<Option<TcpRemoteTransport>>,
  config:        RemoteConfig,
  remote_shared: OnceLock<RemoteShared>,
  event_sender:  OnceLock<Sender<RemoteEvent>>,
  run_state:     Mutex<RemotingRunState>,
}

struct RemotingRunState {
  receiver: Option<TokioMpscRemoteEventReceiver>,
  handle:   Option<JoinHandle<Result<(), RemotingError>>>,
}

impl RemotingRunState {
  const fn new() -> Self {
    Self { receiver: None, handle: None }
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
      run_state: Mutex::new(RemotingRunState::new()),
    }
  }

  /// Returns a clone of the shared [`Remote`] handle.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] when the installer has not been
  /// installed into an actor system yet.
  pub fn remote(&self) -> Result<RemoteShared, RemotingError> {
    self.remote_shared.get().cloned().ok_or(RemotingError::NotStarted)
  }

  /// Spawns the core remote run loop once.
  ///
  /// Must be called from within a Tokio runtime context because it uses
  /// `tokio::spawn` to drive [`RemoteShared::run`]. Call it from async code, a
  /// Tokio task, or a `tokio::Runtime::block_on` context.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] if the installer has not been
  /// installed yet, or [`RemotingError::AlreadyRunning`] if the run loop was
  /// already spawned.
  pub fn spawn_run_task(&self) -> Result<(), RemotingError> {
    let remote = self.remote_shared.get().cloned().ok_or(RemotingError::NotStarted)?;
    let mut run_state = self.run_state.lock().expect(RUN_STATE_LOCK_POISONED);
    if run_state.handle.is_some() {
      return Err(RemotingError::AlreadyRunning);
    }
    let Some(mut receiver) = run_state.receiver.take() else {
      return Err(RemotingError::AlreadyRunning);
    };
    let handle = tokio::spawn(async move { remote.run(&mut receiver).await });
    run_state.handle = Some(handle);
    Ok(())
  }

  /// Shuts the remote subsystem down, wakes the run loop, and waits for it.
  pub async fn shutdown_and_join(&self) -> Result<(), RemotingError> {
    let remote = self.remote_shared.get().cloned().ok_or(RemotingError::NotStarted)?;
    let shutdown_result = remote.shutdown();
    if let Err(error) = &shutdown_result {
      tracing::debug!(?error, "remote shutdown failed; still attempting to join run task");
    }
    if let Some(sender) = self.event_sender.get() {
      // ベストエフォートの wake。Full は pending event が shutdown を観測できる状態、
      // Closed は receiver が既に終了しており join 側がそれを観測する状態として扱う。
      if let Err(send_err) = sender.try_send(RemoteEvent::TransportShutdown) {
        tracing::debug!(?send_err, "shutdown wake failed");
      }
    }
    let handle = {
      let mut run_state = self.run_state.lock().expect(RUN_STATE_LOCK_POISONED);
      run_state.handle.take()
    };
    let Some(handle) = handle else {
      return shutdown_result;
    };
    let join_result = join_run_handle(handle).await;
    match (shutdown_result, join_result) {
      | (shutdown_result, Err(error)) => {
        if let Err(shutdown_error) = shutdown_result {
          tracing::warn!(?shutdown_error, ?error, "remote shutdown failed before run task join failed");
        }
        Err(error)
      },
      | (Err(error), Ok(())) => Err(error),
      | (Ok(()), Ok(())) => Ok(()),
    }
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
    let mut run_state =
      self.run_state.lock().map_err(|_| ActorSystemBuildError::Configuration(String::from(RUN_STATE_LOCK_POISONED)))?;
    run_state.receiver = Some(TokioMpscRemoteEventReceiver::new(event_receiver));
    self
      .event_sender
      .set(event_sender)
      .map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))?;
    // ExtensionInstaller::install は &self 契約のため、一回限りの初期化に OnceLock を使う。
    self.remote_shared.set(remote).map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))
  }
}

async fn join_run_handle(handle: JoinHandle<Result<(), RemotingError>>) -> Result<(), RemotingError> {
  match handle.await {
    | Ok(result) => result,
    | Err(join_error) => {
      tracing::error!(?join_error, "remote run task join failed");
      Err(RemotingError::TransportUnavailable)
    },
  }
}
