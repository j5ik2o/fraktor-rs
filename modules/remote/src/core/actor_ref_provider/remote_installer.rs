//! Builder-facing installer for the remote actor-ref provider.

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
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{loopback_router, remote::RemoteActorRefProviderGeneric};
use crate::core::{
  endpoint_reader::EndpointReaderGeneric,
  endpoint_writer::{EndpointWriterGeneric, EndpointWriterSharedGeneric},
  remoting_extension::RemotingExtensionGeneric,
};

/// Installer registered via the actor system builder's `with_actor_ref_provider` method.
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

    let Some(serialization_arc) = extended.extension_by_type::<SerializationExtensionSharedGeneric<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("serialization extension not installed".into()));
    };
    let serialization = (*serialization_arc).clone();

    let writer = EndpointWriterSharedGeneric::new(EndpointWriterGeneric::new(system.downgrade(), serialization));

    let Some(extension) = extended.extension_by_type::<RemotingExtensionGeneric<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("remoting extension not installed".into()));
    };

    let control = extension.handle();
    let provider = RemoteActorRefProviderGeneric::from_components(system.clone(), writer, control)
      .map_err(|error| ActorSystemBuildError::Configuration(format!("{error}")))?;
    let shared = RemoteWatchHookShared::new(provider, &[ActorPathScheme::FraktorTcp]);
    let shared_provider = ActorRefProviderSharedGeneric::new(shared.clone());
    extended.register_actor_ref_provider(&shared_provider)?;
    extended.register_remote_watch_hook(shared);

    if self.enable_loopback {
      let Some(authority) = system.canonical_authority() else {
        return Err(ActorSystemBuildError::Configuration("canonical authority missing for loopback routing".into()));
      };
      let Some(serialization_ext_arc) = extended.extension_by_type::<SerializationExtensionSharedGeneric<TB>>() else {
        return Err(ActorSystemBuildError::Configuration(
          "serialization extension missing for loopback routing".into(),
        ));
      };
      let reader = EndpointReaderGeneric::new(system.downgrade(), (*serialization_ext_arc).clone());
      loopback_router::register_endpoint(authority, reader, system.clone());
    }
    Ok(())
  }
}
