//! Builder-facing installer for the remote actor-ref provider.

use alloc::{
  format,
  string::{String, ToString},
};

use fraktor_actor_rs::core::{
  logging::LogLevel,
  serialization::{
    SerializationExtensionGeneric, SerializationSetup, SerializationSetupBuilder, SerializerId, StringSerializer,
  },
  system::{ActorRefProviderInstaller, ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  endpoint_reader::EndpointReader, endpoint_writer::EndpointWriter, loopback_router,
  remote_actor_ref_provider::RemoteActorRefProviderGeneric, remoting_extension::RemotingExtension,
};

/// Installer registered via [`ActorSystemBuilder::with_actor_ref_provider`].
pub struct RemoteActorRefProviderInstaller<TB: RuntimeToolbox + 'static> {
  setup:   SerializationSetup,
  _marker: core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> RemoteActorRefProviderInstaller<TB> {
  /// Creates an installer that uses the default loopback serialization setup (String serializer
  /// only).
  #[must_use]
  pub fn loopback() -> Self {
    Self::with_serialization_setup(default_loopback_setup())
  }

  /// Creates an installer with the provided serialization setup.
  #[must_use]
  pub fn with_serialization_setup(setup: SerializationSetup) -> Self {
    Self { setup, _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefProviderInstaller<TB> for RemoteActorRefProviderInstaller<TB> {
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let serialization = ArcShared::new(SerializationExtensionGeneric::new(system, self.setup.clone()));
    let writer = ArcShared::new(EndpointWriter::new(system.clone(), serialization));
    let extended = system.extended();
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

    if extension.transport_scheme() == loopback_router::scheme() {
      if let Some(authority) = system.canonical_authority() {
        if let Some(serialization_ext) = extended.extension_by_type::<SerializationExtensionGeneric<TB>>() {
          let reader = EndpointReader::new(system.clone(), serialization_ext);
          loopback_router::register_endpoint(authority, reader, system.clone());
        } else {
          system.emit_log(
            LogLevel::Warn,
            "serialization extension missing; loopback routing disabled".to_string(),
            None,
          );
        }
      } else {
        system.emit_log(LogLevel::Warn, "canonical authority missing; loopback routing disabled".to_string(), None);
      }
    }
    Ok(())
  }
}

fn default_loopback_setup() -> SerializationSetup {
  let serializer_id = SerializerId::try_from(81).expect("serializer id");
  let serializer: ArcShared<dyn fraktor_actor_rs::core::serialization::Serializer> =
    ArcShared::new(StringSerializer::new(serializer_id));
  SerializationSetupBuilder::new()
    .register_serializer("string", serializer_id, serializer)
    .expect("register serializer")
    .bind::<String>("string")
    .expect("bind string")
    .bind_remote_manifest::<String>("remote.String")
    .expect("manifest")
    .set_fallback("string")
    .expect("fallback")
    .require_manifest_for_scope(fraktor_actor_rs::core::serialization::SerializationCallScope::Remote)
    .build()
    .expect("build setup")
}
