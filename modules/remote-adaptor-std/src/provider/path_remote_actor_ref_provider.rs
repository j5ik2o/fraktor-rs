//! Stateless remote-only actor-ref provider for canonical remote paths.

use fraktor_actor_core_kernel_rs::actor::{
  Pid,
  actor_path::{ActorPath, ActorPathScheme},
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId},
  config::RemoteConfig,
  extension::RemoteShared,
  provider::{ProviderError, RemoteActorRef, RemoteActorRefProvider, resolve_remote_address},
};

/// Resolves remote `fraktor.tcp` actor paths into data-only remote refs.
pub struct PathRemoteActorRefProvider {
  config:        RemoteConfig,
  remote_shared: Option<RemoteShared>,
}

impl PathRemoteActorRefProvider {
  /// Creates a provider using `config`'s automatic remote peer allowlist.
  #[must_use]
  pub fn new(config: RemoteConfig) -> Self {
    Self { config, remote_shared: None }
  }

  /// Creates a provider using `config`'s allowlist plus peers explicitly
  /// connected through `remote_shared`.
  #[must_use]
  pub fn new_with_remote(config: RemoteConfig, remote_shared: RemoteShared) -> Self {
    Self { config, remote_shared: Some(remote_shared) }
  }
}

impl Default for PathRemoteActorRefProvider {
  fn default() -> Self {
    Self::new(RemoteConfig::new(""))
  }
}

impl RemoteActorRefProvider for PathRemoteActorRefProvider {
  fn actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError> {
    if path.parts().scheme() != ActorPathScheme::FraktorTcp {
      return Err(ProviderError::UnsupportedScheme);
    }
    let unique = resolve_remote_address(&path).ok_or(ProviderError::MissingAuthority)?;
    let address = unique.address();
    if !self.is_remote_peer_accepted(address) {
      return Err(ProviderError::RemotePeerNotAllowed);
    }
    let port = if address.port() == 0 { None } else { Some(address.port()) };
    let node = RemoteNodeId::new(address.system(), address.host(), port, unique.uid());
    Ok(RemoteActorRef::new(path, node))
  }

  fn watch(&mut self, watchee: ActorPath, _watcher: Pid) -> Result<(), ProviderError> {
    self.remote_path_result(&watchee)
  }

  fn unwatch(&mut self, watchee: ActorPath, _watcher: Pid) -> Result<(), ProviderError> {
    self.remote_path_result(&watchee)
  }
}

impl PathRemoteActorRefProvider {
  fn remote_path_result(&self, path: &ActorPath) -> Result<(), ProviderError> {
    if path.parts().scheme() != ActorPathScheme::FraktorTcp {
      return Err(ProviderError::UnsupportedScheme);
    }
    let unique = resolve_remote_address(path).ok_or(ProviderError::MissingAuthority)?;
    if self.is_remote_peer_accepted(unique.address()) { Ok(()) } else { Err(ProviderError::RemotePeerNotAllowed) }
  }

  fn is_remote_peer_accepted(&self, remote: &Address) -> bool {
    self.config.is_remote_peer_allowed(remote)
      || self.remote_shared.as_ref().is_some_and(|remote_shared| remote_shared.is_explicit_peer(remote))
  }
}
