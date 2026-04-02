//! Installer for local-only actor-ref provider.

use super::{
  actor_ref_provider_installer::ActorRefProviderInstaller, actor_ref_provider_shared::ActorRefProviderShared,
  local_actor_ref_provider::LocalActorRefProvider,
};
use crate::core::kernel::system::{ActorSystem, ActorSystemBuildError};

/// Installer for local-only actor-ref provider.
///
/// This installer is used when the actor system should only support local actor references,
/// without any remoting capabilities. This is the default behavior of ActorSystem when
/// no ActorRefProviderInstaller is specified.
///
/// Using this installer makes the local-only intent explicit in the configuration.
pub struct LocalActorRefProviderInstaller {
  _marker: core::marker::PhantomData<()>,
}

impl Default for LocalActorRefProviderInstaller {
  fn default() -> Self {
    Self { _marker: core::marker::PhantomData }
  }
}

impl ActorRefProviderInstaller for LocalActorRefProviderInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let provider = ActorRefProviderShared::new(LocalActorRefProvider::new_with_state(system.state()));
    system.extended().register_actor_ref_provider(&provider)?;
    Ok(())
  }
}
