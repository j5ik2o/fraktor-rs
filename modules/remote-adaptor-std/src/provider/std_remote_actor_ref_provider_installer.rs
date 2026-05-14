//! Actor-system config installer for the std remote actor-ref provider.

use std::{boxed::Box, string::String, sync::Mutex, time::Instant};

use fraktor_actor_core_kernel_rs::{
  actor::actor_ref_provider::{ActorRefProviderHandleShared, ActorRefProviderInstaller, LocalActorRefProvider},
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::{
  address::UniqueAddress,
  extension::{EventPublisher, RemoteEvent},
  provider::RemoteActorRefProvider,
  watcher::WatcherCommand,
};
use fraktor_utils_core_rs::sync::ArcShared;
use tokio::sync::mpsc::Sender;

use super::{
  StdRemoteActorRefProvider, path_remote_actor_ref_provider::PathRemoteActorRefProvider,
  remote_actor_path_registry::RemoteActorPathRegistry, remote_watch_hook::StdRemoteWatchHook,
};
use crate::extension_installer::RemotingExtensionInstaller;

const PROVIDER_ALREADY_INSTALLED: &str = "std remote actor-ref provider installer was already consumed";
const PROVIDER_LOCK_POISONED: &str = "std remote actor-ref provider installer lock poisoned";

/// Installs [`StdRemoteActorRefProvider`] through `ActorSystemConfig`.
pub struct StdRemoteActorRefProviderInstaller {
  local_address:      UniqueAddress,
  remote_provider:    Mutex<Option<Box<dyn RemoteActorRefProvider + Send + Sync>>>,
  remoting_installer: ArcShared<RemotingExtensionInstaller>,
}

impl StdRemoteActorRefProviderInstaller {
  /// Creates an installer that enqueues into a config-installed remoting extension.
  #[must_use]
  pub fn from_remoting_extension_installer(
    local_address: UniqueAddress,
    remoting_installer: ArcShared<RemotingExtensionInstaller>,
  ) -> Self {
    Self { local_address, remote_provider: Mutex::new(Some(Box::new(PathRemoteActorRefProvider))), remoting_installer }
  }

  fn event_sender_epoch_and_watcher(
    &self,
  ) -> Result<(Sender<RemoteEvent>, Instant, Sender<WatcherCommand>), ActorSystemBuildError> {
    self
      .remoting_installer
      .remote_event_sender_epoch_and_watcher()
      .map_err(|error| ActorSystemBuildError::Configuration(error.to_string()))
  }
}

impl ActorRefProviderInstaller for StdRemoteActorRefProviderInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let mut remote_provider = self
      .remote_provider
      .lock()
      .map_err(|_| ActorSystemBuildError::Configuration(String::from(PROVIDER_LOCK_POISONED)))?;
    let Some(remote_provider) = remote_provider.take() else {
      return Err(ActorSystemBuildError::Configuration(String::from(PROVIDER_ALREADY_INSTALLED)));
    };
    let (event_sender, monotonic_epoch, watcher_sender) = self.event_sender_epoch_and_watcher()?;
    let local_provider = ActorRefProviderHandleShared::new(LocalActorRefProvider::new_with_state(&system.state()));
    let registry = RemoteActorPathRegistry::new_shared();
    let provider = StdRemoteActorRefProvider::new_with_registry(
      self.local_address.clone(),
      local_provider,
      remote_provider,
      event_sender.clone(),
      EventPublisher::new(system.downgrade()),
      registry.clone(),
      monotonic_epoch,
    );
    let provider = ActorRefProviderHandleShared::new(provider);
    system.extended().register_actor_ref_provider(&provider)?;
    system.extended().register_remote_watch_hook(StdRemoteWatchHook::new(
      registry,
      system.state(),
      event_sender,
      watcher_sender,
      monotonic_epoch,
    ));
    Ok(())
  }
}
