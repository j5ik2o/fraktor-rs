//! Installer for local-only actor-ref provider.

use core::marker::PhantomData;

use super::{
  ActorRefProviderHandleShared, actor_ref_provider_installer::ActorRefProviderInstaller,
  local_actor_ref_provider::LocalActorRefProvider,
};
use crate::system::{ActorSystem, ActorSystemBuildError};

/// Installer for local-only actor-ref provider.
///
/// This installer is used when the actor system should only support local actor references,
/// without any remoting capabilities. This is the default behavior of ActorSystem when
/// no ActorRefProviderInstaller is specified.
///
/// Using this installer makes the local-only intent explicit in the configuration.
pub struct LocalActorRefProviderInstaller {
  _marker: PhantomData<()>,
}

impl Default for LocalActorRefProviderInstaller {
  fn default() -> Self {
    Self { _marker: PhantomData }
  }
}

impl ActorRefProviderInstaller for LocalActorRefProviderInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let actor_ref_provider_handle_shared =
      ActorRefProviderHandleShared::new(LocalActorRefProvider::new_with_state(&system.state()));
    system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
  }
}
