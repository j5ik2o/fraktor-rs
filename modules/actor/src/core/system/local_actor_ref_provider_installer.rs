//! Installer for local-only actor-ref provider.

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::system::{
  ActorRefProviderInstaller, ActorSystemBuildError, ActorSystemGeneric, LocalActorRefProviderGeneric,
};

/// Installer for local-only actor-ref provider.
///
/// This installer is used when the actor system should only support local actor references,
/// without any remoting capabilities. This is the default behavior of ActorSystem when
/// no ActorRefProviderInstaller is specified.
///
/// Using this installer makes the local-only intent explicit in the configuration.
pub struct LocalActorRefProviderInstaller<TB: RuntimeToolbox + 'static> {
  _marker: core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> Default for LocalActorRefProviderInstaller<TB> {
  fn default() -> Self {
    Self { _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefProviderInstaller<TB> for LocalActorRefProviderInstaller<TB> {
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let provider = ArcShared::new(LocalActorRefProviderGeneric::<TB>::new());
    system.extended().register_actor_ref_provider(&provider);
    Ok(())
  }
}
