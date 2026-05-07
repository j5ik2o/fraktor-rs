//! Actor-system config installer for the std remote actor-ref provider.

use std::{boxed::Box, string::String, sync::Mutex, time::Instant};

use fraktor_actor_core_rs::core::kernel::{
  actor::actor_ref_provider::{ActorRefProviderHandleShared, ActorRefProviderInstaller, LocalActorRefProvider},
  serialization::ActorRefResolveCache,
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_remote_core_rs::core::{
  address::UniqueAddress,
  extension::{EventPublisher, RemoteEvent},
  provider::RemoteActorRefProvider,
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::sync::mpsc::Sender;

use super::StdRemoteActorRefProvider;
use crate::std::extension_installer::RemotingExtensionInstaller;

const PROVIDER_ALREADY_INSTALLED: &str = "std remote actor-ref provider installer was already consumed";
const PROVIDER_LOCK_POISONED: &str = "std remote actor-ref provider installer lock poisoned";

/// Installs [`StdRemoteActorRefProvider`] through `ActorSystemConfig`.
pub struct StdRemoteActorRefProviderInstaller {
  local_address:   UniqueAddress,
  remote_provider: Mutex<Option<Box<dyn RemoteActorRefProvider + Send + Sync>>>,
  event_source:    RemoteEventSource,
}

enum RemoteEventSource {
  Fixed { event_sender: Sender<RemoteEvent>, monotonic_epoch: Instant },
  RemotingInstaller(ArcShared<RemotingExtensionInstaller>),
}

impl StdRemoteActorRefProviderInstaller {
  /// Creates a config installer using the provided remote-only provider and event sender.
  #[must_use]
  pub fn new(
    local_address: UniqueAddress,
    remote_provider: Box<dyn RemoteActorRefProvider + Send + Sync>,
    event_sender: Sender<RemoteEvent>,
    monotonic_epoch: Instant,
  ) -> Self {
    Self {
      local_address,
      remote_provider: Mutex::new(Some(remote_provider)),
      event_source: RemoteEventSource::Fixed { event_sender, monotonic_epoch },
    }
  }

  /// Creates an installer that enqueues into a config-installed remoting extension.
  #[must_use]
  pub fn from_remoting_extension_installer(
    local_address: UniqueAddress,
    remote_provider: Box<dyn RemoteActorRefProvider + Send + Sync>,
    remoting_installer: ArcShared<RemotingExtensionInstaller>,
  ) -> Self {
    Self {
      local_address,
      remote_provider: Mutex::new(Some(remote_provider)),
      event_source: RemoteEventSource::RemotingInstaller(remoting_installer),
    }
  }

  fn event_sender_and_epoch(&self) -> Result<(Sender<RemoteEvent>, Instant), ActorSystemBuildError> {
    match &self.event_source {
      | RemoteEventSource::Fixed { event_sender, monotonic_epoch } => Ok((event_sender.clone(), *monotonic_epoch)),
      | RemoteEventSource::RemotingInstaller(installer) => installer
        .remote_event_sender_and_epoch()
        .map_err(|error| ActorSystemBuildError::Configuration(error.to_string())),
    }
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
    let (event_sender, monotonic_epoch) = self.event_sender_and_epoch()?;
    let local_provider = ActorRefProviderHandleShared::new(LocalActorRefProvider::new_with_state(&system.state()));
    let provider = StdRemoteActorRefProvider::new(
      self.local_address.clone(),
      local_provider,
      remote_provider,
      event_sender,
      ActorRefResolveCache::default(),
      EventPublisher::new(system.downgrade()),
      monotonic_epoch,
    );
    let provider = ActorRefProviderHandleShared::new(provider);
    system.extended().register_actor_ref_provider(&provider)
  }
}
