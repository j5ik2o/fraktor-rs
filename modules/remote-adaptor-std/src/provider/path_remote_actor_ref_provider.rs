//! Stateless remote-only actor-ref provider for canonical remote paths.

use fraktor_actor_core_kernel_rs::actor::{
  Pid,
  actor_path::{ActorPath, ActorPathScheme},
};
use fraktor_remote_core_rs::{
  address::RemoteNodeId,
  provider::{ProviderError, RemoteActorRef, RemoteActorRefProvider, resolve_remote_address},
};

/// Resolves remote `fraktor.tcp` actor paths into data-only remote refs.
#[derive(Default)]
pub struct PathRemoteActorRefProvider;

impl RemoteActorRefProvider for PathRemoteActorRefProvider {
  fn actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError> {
    if path.parts().scheme() != ActorPathScheme::FraktorTcp {
      return Err(ProviderError::UnsupportedScheme);
    }
    let unique = resolve_remote_address(&path).ok_or(ProviderError::MissingAuthority)?;
    let address = unique.address();
    let port = if address.port() == 0 { None } else { Some(address.port()) };
    let node = RemoteNodeId::new(address.system(), address.host(), port, unique.uid());
    Ok(RemoteActorRef::new(path, node))
  }

  fn watch(&mut self, watchee: ActorPath, _watcher: Pid) -> Result<(), ProviderError> {
    remote_path_result(&watchee)
  }

  fn unwatch(&mut self, watchee: ActorPath, _watcher: Pid) -> Result<(), ProviderError> {
    remote_path_result(&watchee)
  }
}

fn remote_path_result(path: &ActorPath) -> Result<(), ProviderError> {
  if path.parts().scheme() != ActorPathScheme::FraktorTcp {
    return Err(ProviderError::UnsupportedScheme);
  }
  resolve_remote_address(path).map(|_| ()).ok_or(ProviderError::MissingAuthority)
}
