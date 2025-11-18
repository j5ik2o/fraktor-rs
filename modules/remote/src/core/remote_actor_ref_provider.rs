//! Remote actor-ref provider wiring remoting control, watcher daemon, and quickstart flows.

use alloc::{format, vec::Vec};

use fraktor_actor_rs::core::{
  actor_prim::actor_path::ActorPathParts, messaging::AnyMessageGeneric, system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};
#[cfg(feature = "std")]
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  RemotingConnectionSnapshot, RemotingControl, RemotingControlHandle, RemotingError,
  core::{
    remote_actor_ref_provider_setup::RemoteActorRefProviderSetup,
    remote_watcher_daemon::{RemoteWatcherDaemon, RemoteWatcherMessage},
  },
};

/// Provides convenience APIs for integrating remoting with actor systems.
pub struct RemoteActorRefProvider<TB: RuntimeToolbox + 'static> {
  system:  ActorSystemGeneric<TB>,
  control: RemotingControlHandle<TB>,
  watcher: fraktor_actor_rs::core::actor_prim::actor_ref::ActorRefGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for RemoteActorRefProvider<TB> {
  fn clone(&self) -> Self {
    Self { system: self.system.clone(), control: self.control.clone(), watcher: self.watcher.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> RemoteActorRefProvider<TB> {
  /// Creates a new provider and spawns the remote watcher daemon.
  pub fn new(system: &ActorSystemGeneric<TB>, control: RemotingControlHandle<TB>) -> Result<Self, RemotingError> {
    let watcher = RemoteWatcherDaemon::spawn(system, control.clone())?;
    Ok(Self { system: system.clone(), control, watcher })
  }

  /// Returns the underlying remoting control handle.
  #[must_use]
  pub fn handle(&self) -> RemotingControlHandle<TB> {
    self.control.clone()
  }

  /// Registers a remote watch request for the specified actor path parts.
  pub fn watch_remote(&self, target: ActorPathParts) -> Result<(), RemotingError> {
    self
      .watcher
      .tell(AnyMessageGeneric::new(RemoteWatcherMessage::Watch { target }))
      .map_err(|err| RemotingError::message(format!("{err:?}")))
  }

  /// Cancels a previously registered remote watch.
  pub fn unwatch_remote(&self, target: ActorPathParts) -> Result<(), RemotingError> {
    self
      .watcher
      .tell(AnyMessageGeneric::new(RemoteWatcherMessage::Unwatch { target }))
      .map_err(|err| RemotingError::message(format!("{err:?}")))
  }

  /// Returns the latest connection snapshot aggregated by the flight recorder.
  #[must_use]
  pub fn connections_snapshot(&self) -> Vec<RemotingConnectionSnapshot> {
    self.control.connections_snapshot()
  }

  /// Returns the underlying actor system.
  #[must_use]
  pub fn system(&self) -> &ActorSystemGeneric<TB> {
    &self.system
  }
}

impl RemoteActorRefProvider<NoStdToolbox> {
  /// Returns the installer for loopback/no_std environments.
  #[must_use]
  pub fn loopback() -> RemoteActorRefProviderSetup<NoStdToolbox> {
    RemoteActorRefProviderSetup::new()
  }
}

#[cfg(feature = "std")]
impl RemoteActorRefProvider<StdToolbox> {
  /// Returns the installer for std/Tokio environments.
  #[must_use]
  pub fn std() -> RemoteActorRefProviderSetup<StdToolbox> {
    RemoteActorRefProviderSetup::new()
  }
}
