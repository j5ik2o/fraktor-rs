//! Installer for the serialization extension.

use alloc::format;

use crate::core::{
  extension::ExtensionInstaller,
  serialization::{SerializationExtensionId, SerializationSetup},
  system::{ActorSystem, ActorSystemBuildError},
};

/// Installer that registers the serialization extension during actor system bootstrap.
pub struct SerializationExtensionInstaller {
  setup: SerializationSetup,
}

impl SerializationExtensionInstaller {
  /// Creates a new serialization extension installer with the provided setup.
  #[must_use]
  pub const fn new(setup: SerializationSetup) -> Self {
    Self { setup }
  }

  /// Returns the serialization setup.
  #[must_use]
  pub const fn setup(&self) -> &SerializationSetup {
    &self.setup
  }
}

impl ExtensionInstaller for SerializationExtensionInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let extension_id = SerializationExtensionId::new(self.setup.clone());
    system.extended().register_extension(&extension_id).map(|_| ()).map_err(|error| {
      ActorSystemBuildError::Configuration(format!("serialization extension registration failed: {error:?}"))
    })?;
    Ok(())
  }
}
