//! Installer for the serialization extension.

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::kernel::{
  actor::extension::ExtensionInstaller,
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
    let registered = system.extended().register_extension(&extension_id);
    let existing = system
      .extended()
      .extension(&extension_id)
      .ok_or_else(|| ActorSystemBuildError::Configuration("serialization extension was not retained".into()))?;
    if !ArcShared::ptr_eq(&registered, &existing) {
      return Err(ActorSystemBuildError::Configuration("serialization extension identity mismatch".into()));
    }
    Ok(())
  }
}
