//! Builder-facing installer for the remote actor-ref provider.

use alloc::format;

use fraktor_actor_rs::core::{
  serialization::SerializationExtensionGeneric,
  system::{ActorRefProviderInstaller, ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  EndpointWriterGeneric, endpoint_reader::EndpointReader, loopback_router,
  remote_actor_ref_provider::RemoteActorRefProviderGeneric, remoting_extension::RemotingExtension,
};

/// Installer registered via [`ActorSystemBuilder::with_actor_ref_provider`].
pub struct RemoteActorRefProviderInstaller<TB: RuntimeToolbox + 'static> {
  enable_loopback: bool,
  _marker:         core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> RemoteActorRefProviderInstaller<TB> {
  /// Creates a remote actor-ref provider installer with loopback routing enabled.
  #[must_use]
  pub fn loopback() -> Self {
    Self { enable_loopback: true, _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for RemoteActorRefProviderInstaller<TB> {
  fn default() -> Self {
    Self { enable_loopback: false, _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefProviderInstaller<TB> for RemoteActorRefProviderInstaller<TB> {
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let extended = system.extended();

    let Some(serialization) = extended.extension_by_type::<SerializationExtensionGeneric<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("serialization extension not installed".into()));
    };

    let writer = ArcShared::new(EndpointWriterGeneric::new(system.clone(), serialization));

    let Some(extension) = extended.extension_by_type::<RemotingExtension<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("remoting extension not installed".into()));
    };

    let control = extension.handle();
    let authority_manager = system.state().remote_authority_manager().clone();
    let provider = RemoteActorRefProviderGeneric::from_components(system.clone(), writer, control, authority_manager)
      .map_err(|error| ActorSystemBuildError::Configuration(format!("{error}")))?;
    let provider = ArcShared::new(provider);
    extended.register_actor_ref_provider(provider.clone());
    extended.register_remote_watch_hook(provider.clone());

    if self.enable_loopback {
      let Some(authority) = system.canonical_authority() else {
        return Err(ActorSystemBuildError::Configuration("canonical authority missing for loopback routing".into()));
      };
      let Some(serialization_ext) = extended.extension_by_type::<SerializationExtensionGeneric<TB>>() else {
        return Err(ActorSystemBuildError::Configuration(
          "serialization extension missing for loopback routing".into(),
        ));
      };
      let reader = EndpointReader::new(system.clone(), serialization_ext);
      loopback_router::register_endpoint(authority, reader, system.clone());
    }
    Ok(())
  }
}
