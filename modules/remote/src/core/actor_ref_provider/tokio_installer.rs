//! Builder-facing installer for the Tokio TCP actor-ref provider.

use alloc::format;

use fraktor_actor_rs::core::{
  actor::actor_path::ActorPathScheme,
  serialization::SerializationExtensionSharedGeneric,
  system::{
    ActorRefProviderInstaller, ActorRefProviderSharedGeneric, ActorSystemBuildError, ActorSystemGeneric,
    RemoteWatchHookShared,
  },
};
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{loopback_router, tokio::TokioActorRefProviderGeneric};
use crate::core::{
  EndpointReaderGeneric, EndpointWriterGeneric, EndpointWriterSharedGeneric, RemotingExtensionGeneric,
  transport::TokioTransportConfig,
};

/// Installer for Tokio TCP actor-ref provider.
pub struct TokioActorRefProviderInstaller<TB: RuntimeToolbox + 'static> {
  transport_config: TokioTransportConfig,
  enable_loopback:  bool,
  _marker:          core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> TokioActorRefProviderInstaller<TB> {
  /// Creates a new Tokio actor-ref provider installer.
  #[must_use]
  pub fn new(transport_config: TokioTransportConfig, enable_loopback: bool) -> Self {
    Self { transport_config, enable_loopback, _marker: core::marker::PhantomData }
  }

  /// Creates a Tokio TCP transport installer (loopback routing disabled).
  #[must_use]
  pub fn from_config(transport_config: TokioTransportConfig) -> Self {
    Self::new(transport_config, false)
  }

  /// Creates a Tokio transport installer with loopback routing enabled.
  #[must_use]
  pub fn from_config_with_loopback(transport_config: TokioTransportConfig) -> Self {
    Self::new(transport_config, true)
  }
}

impl<TB: RuntimeToolbox + 'static> Default for TokioActorRefProviderInstaller<TB> {
  fn default() -> Self {
    Self::from_config(TokioTransportConfig::default())
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
    let provider =
      TokioActorRefProviderGeneric::from_components(system.clone(), writer, control, self.transport_config.clone())
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
