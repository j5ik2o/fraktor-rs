//! Builder-facing installer for the Tokio TCP actor-ref provider.

use alloc::format;

use fraktor_actor_rs::core::kernel::{
  actor::{
    actor_path::ActorPathScheme,
    actor_ref_provider::{ActorRefProviderInstaller, ActorRefProviderShared},
  },
  serialization::SerializationExtensionShared,
  system::{ActorSystem, ActorSystemBuildError, remote::RemoteWatchHookShared},
};
use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  actor_ref_provider::{loopback_router, tokio::TokioActorRefProvider},
  endpoint_reader::EndpointReader,
  endpoint_writer::{EndpointWriter, EndpointWriterShared},
  remoting_extension::RemotingExtension,
};

/// Installer for Tokio TCP actor-ref provider.
#[derive(Default)]
pub struct TokioActorRefProviderInstaller {
  enable_loopback: bool,
}

impl TokioActorRefProviderInstaller {
  /// Creates a Tokio actor-ref provider installer with loopback routing enabled.
  #[must_use]
  pub fn loopback() -> Self {
    Self { enable_loopback: true }
  }
}

impl ActorRefProviderInstaller for TokioActorRefProviderInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let extended = system.extended();

    let Some(serialization_arc) = extended.extension_by_type::<SerializationExtensionShared>() else {
      return Err(ActorSystemBuildError::Configuration("serialization extension not installed".into()));
    };
    let serialization = (*serialization_arc).clone();

    let writer = EndpointWriterShared::new(EndpointWriter::new(system.downgrade(), serialization.clone()));
    let reader = ArcShared::new(EndpointReader::new(system.downgrade(), serialization));

    let Some(extension) = extended.extension_by_type::<RemotingExtension>() else {
      return Err(ActorSystemBuildError::Configuration("remoting extension not installed".into()));
    };

    let control = extension.handle();
    control.lock().register_endpoint_io(writer.clone(), reader.clone());
    let provider = TokioActorRefProvider::from_components(system.clone(), writer, control)
      .map_err(|error| ActorSystemBuildError::Configuration(format!("{error}")))?;
    let shared = RemoteWatchHookShared::new(provider, &[ActorPathScheme::FraktorTcp]);
    let shared_provider = ActorRefProviderShared::new(shared.clone());
    extended.register_actor_ref_provider(&shared_provider)?;
    extended.register_remote_watch_hook(shared);

    if self.enable_loopback {
      let Some(authority) = system.canonical_authority() else {
        return Err(ActorSystemBuildError::Configuration("canonical authority missing for loopback routing".into()));
      };
      loopback_router::register_endpoint(authority, (*reader).clone(), system.clone());
    }
    Ok(())
  }
}
