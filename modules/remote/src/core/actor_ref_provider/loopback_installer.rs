//! Builder-facing installer for the Loopback actor-ref provider.

use alloc::format;

use fraktor_actor_rs::core::{
  actor::actor_path::ActorPathScheme,
  serialization::SerializationExtensionSharedGeneric,
  system::{
    ActorSystemBuildError, ActorSystemGeneric,
    provider::{ActorRefProviderInstaller, ActorRefProviderSharedGeneric},
    remote::RemoteWatchHookShared,
  },
};
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{loopback::LoopbackActorRefProviderGeneric, loopback_router};
use crate::core::{
  endpoint_reader::EndpointReaderGeneric,
  endpoint_writer::{EndpointWriterGeneric, EndpointWriterSharedGeneric},
  remoting_extension::RemotingExtensionGeneric,
};

/// Installer for Loopback actor-ref provider.
pub struct LoopbackActorRefProviderInstaller<TB: RuntimeToolbox + 'static> {
  _marker: core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> LoopbackActorRefProviderInstaller<TB> {
  /// Creates a new Loopback actor-ref provider installer.
  #[must_use]
  pub const fn new() -> Self {
    Self { _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for LoopbackActorRefProviderInstaller<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefProviderInstaller<TB> for LoopbackActorRefProviderInstaller<TB> {
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let extended = system.extended();

    let Some(serialization_arc) = extended.extension_by_type::<SerializationExtensionSharedGeneric<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("serialization extension not installed".into()));
    };
    let serialization = (*serialization_arc).clone();

    let writer =
      EndpointWriterSharedGeneric::new(EndpointWriterGeneric::new(system.downgrade(), serialization.clone()));
    let reader = ArcShared::new(EndpointReaderGeneric::new(system.downgrade(), serialization));

    let Some(extension) = extended.extension_by_type::<RemotingExtensionGeneric<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("remoting extension not installed".into()));
    };

    let control = extension.handle();
    control.lock().register_endpoint_io(writer.clone(), reader.clone());
    let provider = LoopbackActorRefProviderGeneric::from_components(system.clone(), writer, control)
      .map_err(|error| ActorSystemBuildError::Configuration(format!("{error}")))?;
    let shared = RemoteWatchHookShared::new(provider, &[ActorPathScheme::FraktorTcp]);
    let shared_provider = ActorRefProviderSharedGeneric::new(shared.clone());
    extended.register_actor_ref_provider(&shared_provider)?;
    extended.register_remote_watch_hook(shared);

    // Always register loopback routing for LoopbackActorRefProvider
    let Some(authority) = system.canonical_authority() else {
      return Err(ActorSystemBuildError::Configuration("canonical authority missing for loopback routing".into()));
    };
    loopback_router::register_endpoint(authority, (*reader).clone(), system.clone());
    Ok(())
  }
}
