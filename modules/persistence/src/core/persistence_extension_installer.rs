//! Installer for the persistence extension.

use alloc::format;

use fraktor_actor_rs::core::{
  extension::ExtensionInstaller,
  system::{ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::persistence_extension_id::PersistenceExtensionId;

/// Installer that registers the persistence extension during actor system bootstrap.
#[derive(Clone, Debug, Default)]
pub struct PersistenceExtensionInstaller {
  extension_id: PersistenceExtensionId,
}

impl PersistenceExtensionInstaller {
  /// Creates a new installer with the provided extension identifier.
  #[must_use]
  pub const fn new(extension_id: PersistenceExtensionId) -> Self {
    Self { extension_id }
  }
}

impl<TB: RuntimeToolbox + 'static> ExtensionInstaller<TB> for PersistenceExtensionInstaller {
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    system.extended().register_extension(&self.extension_id).map(|_| ()).map_err(|error| {
      ActorSystemBuildError::Configuration(format!("persistence extension registration failed: {error:?}"))
    })
  }
}
