//! Installer for [`RemoteActorRefProvider`] used by the actor system builder.

use alloc::format;
use core::marker::PhantomData;

use fraktor_actor_rs::core::{
  extension::Extension,
  system::{ActorRefProviderInstaller, ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::{RemoteActorRefProvider, RemotingExtension};

/// Builder-side installer that wires [`RemoteActorRefProvider`] after bootstrap.
pub struct RemoteActorRefProviderSetup<TB>
where
  TB: RuntimeToolbox + 'static, {
  _marker: PhantomData<TB>,
}

impl<TB> RemoteActorRefProviderSetup<TB>
where
  TB: RuntimeToolbox + 'static,
{
  pub(crate) const fn new() -> Self {
    Self { _marker: PhantomData }
  }
}

impl<TB> ActorRefProviderInstaller<TB> for RemoteActorRefProviderSetup<TB>
where
  TB: RuntimeToolbox + 'static,
  RemotingExtension<TB>: Extension<TB>,
{
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let extension = system
      .extension_by_type::<RemotingExtension<TB>>()
      .ok_or_else(|| ActorSystemBuildError::Configuration("remoting extension missing".into()))?;
    let provider = RemoteActorRefProvider::new(system, extension.handle())
      .map_err(|error| ActorSystemBuildError::Configuration(format!("{error}")))?;
    system.register_actor_ref_provider(ArcShared::new(provider));
    Ok(())
  }
}
