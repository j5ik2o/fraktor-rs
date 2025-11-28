//! Builder-facing installer for the Loopback actor-ref provider.

use alloc::format;

use fraktor_actor_rs::core::{
  actor_prim::actor_path::ActorPathScheme,
  serialization::SerializationExtensionGeneric,
  system::{ActorRefProviderInstaller, ActorSystemBuildError, ActorSystemGeneric, RemoteWatchHookShared},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily},
  sync::ArcShared,
};

use crate::core::{
  EndpointWriterGeneric, RemotingExtensionGeneric, endpoint_reader::EndpointReaderGeneric,
  loopback_actor_ref_provider::LoopbackActorRefProviderGeneric, loopback_router,
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

    let Some(serialization) = extended.extension_by_type::<SerializationExtensionGeneric<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("serialization extension not installed".into()));
    };

    let writer = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(EndpointWriterGeneric::new(
      system.clone(),
      serialization.clone(),
    )));
    let reader = ArcShared::new(EndpointReaderGeneric::new(system.clone(), serialization.clone()));

    let Some(extension) = extended.extension_by_type::<RemotingExtensionGeneric<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("remoting extension not installed".into()));
    };

    let control = extension.handle();
    control.register_endpoint_io(writer.clone(), reader.clone());
    let authority_manager = system.state().remote_authority_manager().clone();
    let provider = LoopbackActorRefProviderGeneric::from_components(system.clone(), writer, control, authority_manager)
      .map_err(|error| ActorSystemBuildError::Configuration(format!("{error}")))?;
    let shared = RemoteWatchHookShared::new(provider, &[ActorPathScheme::FraktorTcp]);
    let shared_arc = ArcShared::new(shared.clone());
    extended.register_actor_ref_provider(&shared_arc);
    extended.register_remote_watch_hook(shared);

    // Always register loopback routing for LoopbackActorRefProvider
    let Some(authority) = system.canonical_authority() else {
      return Err(ActorSystemBuildError::Configuration("canonical authority missing for loopback routing".into()));
    };
    loopback_router::register_endpoint(authority, (*reader).clone(), system.clone());
    Ok(())
  }
}
