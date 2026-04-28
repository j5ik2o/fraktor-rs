//! Actor system extension installer for the new `StdRemoting` aggregate.

use std::sync::OnceLock;

use fraktor_actor_core_rs::core::kernel::{
  actor::extension::ExtensionInstaller,
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::core::{config::RemoteConfig, extension::EventPublisher};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

use crate::std::{extension_installer::base::StdRemoting, tcp_transport::TcpRemoteTransport};

const NOT_INSTALLED: &str = "remoting extension is not installed";
const ALREADY_INSTALLED: &str = "remoting extension is already installed";

/// Extension installer for the `fraktor-remote-adaptor-std-rs` runtime.
pub struct RemotingExtensionInstaller {
  transport: SharedLock<TcpRemoteTransport>,
  config:    RemoteConfig,
  remoting:  OnceLock<SharedLock<StdRemoting>>,
}

impl RemotingExtensionInstaller {
  /// Creates a new installer wrapping the given transport.
  #[must_use]
  pub fn new(transport: SharedLock<TcpRemoteTransport>, config: RemoteConfig) -> Self {
    Self { transport, config, remoting: OnceLock::new() }
  }

  /// Returns a clone of the shared `StdRemoting` handle.
  ///
  /// # Errors
  ///
  /// Returns [`ActorSystemBuildError::Configuration`] when the installer has
  /// not been installed into an actor system yet.
  pub fn remoting(&self) -> Result<SharedLock<StdRemoting>, ActorSystemBuildError> {
    self.remoting.get().cloned().ok_or_else(|| ActorSystemBuildError::Configuration(String::from(NOT_INSTALLED)))
  }
}

impl ExtensionInstaller for RemotingExtensionInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let event_publisher = EventPublisher::new(system.downgrade());
    let remoting = SharedLock::new_with_driver::<DefaultMutex<_>>(StdRemoting::new(
      self.transport.clone(),
      self.config.clone(),
      None,
      event_publisher,
    ));
    // ExtensionInstaller::install は &self 契約のため、一回限りの初期化に OnceLock を使う。
    self.remoting.set(remoting).map_err(|_| ActorSystemBuildError::Configuration(String::from(ALREADY_INSTALLED)))
  }
}
