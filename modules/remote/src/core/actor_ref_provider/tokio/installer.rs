//! Builder-facing installer for the Tokio TCP actor-ref provider.

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

use crate::core::{
  actor_ref_provider::{loopback_router, tokio::TokioActorRefProviderGeneric},
  endpoint_reader::EndpointReaderGeneric,
  endpoint_writer::{EndpointWriterGeneric, EndpointWriterSharedGeneric},
  remoting_extension::RemotingExtensionGeneric,
};

/// Installer for Tokio TCP actor-ref provider.
pub struct TokioActorRefProviderInstaller<TB: RuntimeToolbox + 'static> {
  enable_loopback: bool,
  _marker:         core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> TokioActorRefProviderInstaller<TB> {
  /// Creates a Tokio actor-ref provider installer with loopback routing enabled.
  #[must_use]
  pub fn loopback() -> Self {
    Self { enable_loopback: true, _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for TokioActorRefProviderInstaller<TB> {
  fn default() -> Self {
    Self { enable_loopback: false, _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefProviderInstaller<TB> for TokioActorRefProviderInstaller<TB> {
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
    let provider = TokioActorRefProviderGeneric::from_components(system.clone(), writer, control)
      .map_err(|error| ActorSystemBuildError::Configuration(format!("{error}")))?;
    let shared = RemoteWatchHookShared::new(provider, &[ActorPathScheme::FraktorTcp]);
    let shared_provider = ActorRefProviderSharedGeneric::new(shared.clone());
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
