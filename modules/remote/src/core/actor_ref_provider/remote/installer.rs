//! Builder-facing installer for the remote actor-ref provider.

use alloc::format;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    actor_path::ActorPathScheme,
    actor_ref_provider::{ActorRefProviderInstaller, ActorRefProviderShared},
  },
  serialization::SerializationExtensionShared,
  system::{ActorSystem, ActorSystemBuildError, remote::RemoteWatchHookShared},
};

use crate::core::{
  actor_ref_provider::{loopback_router, remote::RemoteActorRefProvider},
  endpoint_reader::EndpointReader,
  endpoint_writer::{EndpointWriter, EndpointWriterShared},
  remoting_extension::RemotingExtension,
};

/// Installer registered via the actor system builder's `with_actor_ref_provider` method.
#[derive(Default)]
pub struct RemoteActorRefProviderInstaller {
  enable_loopback: bool,
}

impl RemoteActorRefProviderInstaller {
  /// Creates a remote actor-ref provider installer with loopback routing enabled.
  #[must_use]
  pub fn loopback() -> Self {
    Self { enable_loopback: true }
  }
}

impl ActorRefProviderInstaller for RemoteActorRefProviderInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let extended = system.extended();

    let Some(serialization_arc) = extended.extension_by_type::<SerializationExtensionShared>() else {
      return Err(ActorSystemBuildError::Configuration("serialization extension not installed".into()));
    };
    let serialization = (*serialization_arc).clone();

    let writer = EndpointWriterShared::new(EndpointWriter::new(system.downgrade(), serialization));

    let Some(extension) = extended.extension_by_type::<RemotingExtension>() else {
      return Err(ActorSystemBuildError::Configuration("remoting extension not installed".into()));
    };

    let control = extension.handle();
    let provider = RemoteActorRefProvider::from_components(system.clone(), writer, control)
      .map_err(|error| ActorSystemBuildError::Configuration(format!("{error}")))?;
    let shared = RemoteWatchHookShared::new(provider, &[ActorPathScheme::FraktorTcp]);
    let shared_provider = ActorRefProviderShared::new(shared.clone());
    extended.register_actor_ref_provider(&shared_provider)?;
    extended.register_remote_watch_hook(shared);

    if self.enable_loopback {
      let Some(authority) = system.canonical_authority() else {
        return Err(ActorSystemBuildError::Configuration("canonical authority missing for loopback routing".into()));
      };
      let Some(serialization_ext_arc) = extended.extension_by_type::<SerializationExtensionShared>() else {
        return Err(ActorSystemBuildError::Configuration(
          "serialization extension missing for loopback routing".into(),
        ));
      };
      let reader = EndpointReader::new(system.downgrade(), (*serialization_ext_arc).clone());
      loopback_router::register_endpoint(authority, reader, system.clone());
    }
    Ok(())
  }
}
