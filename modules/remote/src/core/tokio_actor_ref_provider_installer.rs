//! Builder-facing installer for the Tokio TCP actor-ref provider.

use alloc::format;

use fraktor_actor_rs::core::{
  serialization::SerializationExtensionGeneric,
  system::{ActorRefProviderInstaller, ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  endpoint_reader::EndpointReader, endpoint_writer::EndpointWriter, loopback_router,
  remoting_extension::RemotingExtension, tokio_actor_ref_provider::TokioActorRefProviderGeneric,
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

    let Some(serialization) = extended.extension_by_type::<SerializationExtensionGeneric<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("serialization extension not installed".into()));
    };

    let writer = ArcShared::new(EndpointWriter::new(system.clone(), serialization));

    let Some(extension) = extended.extension_by_type::<RemotingExtension<TB>>() else {
      return Err(ActorSystemBuildError::Configuration("remoting extension not installed".into()));
    };

    let control = extension.handle();
    let authority_manager = system.state().remote_authority_manager().clone();
    let provider = TokioActorRefProviderGeneric::from_components(
      system.clone(),
      writer,
      control,
      authority_manager,
      self.transport_config.clone(),
    )
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
