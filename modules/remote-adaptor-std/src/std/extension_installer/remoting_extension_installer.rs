//! Actor system extension installer for `remote-core`'s `Remote`.

use std::sync::{Mutex, OnceLock};

use fraktor_actor_core_rs::core::kernel::{
  actor::extension::ExtensionInstaller,
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::core::{
  config::RemoteConfig,
  extension::{EventPublisher, Remote},
};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

use crate::std::tcp_transport::TcpRemoteTransport;

const NOT_INSTALLED: &str = "remote extension is not installed";
const ALREADY_INSTALLED: &str = "remote extension is already installed";
const TRANSPORT_LOCK_POISONED: &str = "remote extension transport lock is poisoned";

/// Extension installer for the `fraktor-remote-adaptor-std-rs` runtime.
pub struct RemotingExtensionInstaller {
  transport: Mutex<Option<TcpRemoteTransport>>,
  config:    RemoteConfig,
  remote:    OnceLock<SharedLock<Remote>>,
}

impl RemotingExtensionInstaller {
  /// Creates a new installer that will move the given transport into
  /// `remote-core`'s [`Remote`] during installation.
  #[must_use]
  pub fn new(transport: TcpRemoteTransport, config: RemoteConfig) -> Self {
    Self { transport: Mutex::new(Some(transport)), config, remote: OnceLock::new() }
  }

  /// Returns a clone of the shared [`Remote`] handle.
  ///
  /// # Errors
  ///
  /// Returns [`ActorSystemBuildError::Configuration`] when the installer has
  /// not been installed into an actor system yet.
  pub fn remote(&self) -> Result<SharedLock<Remote>, ActorSystemBuildError> {
    self.remote.get().cloned().ok_or_else(|| ActorSystemBuildError::Configuration(String::from(NOT_INSTALLED)))
  }
}

impl ExtensionInstaller for RemotingExtensionInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let mut transport_slot =
      self.transport.lock().map_err(|_| ActorSystemBuildError::Configuration(String::from(TRANSPORT_LOCK_POISONED)))?;
    if self.remote.get().is_some() {
      return Err(ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)));
    }
    let Some(transport) = transport_slot.take() else {
      return Err(ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)));
    };
    let event_publisher = EventPublisher::new(system.downgrade());
    let remote =
      SharedLock::new_with_driver::<DefaultMutex<_>>(Remote::new(transport, self.config.clone(), event_publisher));
    // ExtensionInstaller::install は &self 契約のため、一回限りの初期化に OnceLock を使う。
    self.remote.set(remote).map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))
  }
}
