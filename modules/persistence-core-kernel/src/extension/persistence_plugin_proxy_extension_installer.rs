//! Installer for the persistence plugin proxy extension.

#[cfg(test)]
#[path = "persistence_plugin_proxy_extension_installer_test.rs"]
mod tests;

use alloc::format;

use fraktor_actor_core_kernel_rs::{
  actor::extension::{ExtensionInstaller, install_extension_id},
  serialization::contribution::register_serialization_registry_contributor,
  system::{ActorSystem, ActorSystemBuildError},
};

use crate::{
  config::PersistenceConfig, extension::PersistencePluginProxyExtensionId,
  serialization::PersistenceSerializationContributor,
};

/// Installs proxy-backed persistence actors into the actor system.
pub struct PersistencePluginProxyExtensionInstaller {
  settings: PersistenceConfig,
}

impl PersistencePluginProxyExtensionInstaller {
  /// Creates a new proxy extension installer.
  #[must_use]
  pub const fn new() -> Self {
    Self::new_with_settings(PersistenceConfig::default_config())
  }

  /// Creates a new proxy extension installer with explicit persistence settings.
  #[must_use]
  pub const fn new_with_settings(settings: PersistenceConfig) -> Self {
    Self { settings }
  }
}

impl Default for PersistencePluginProxyExtensionInstaller {
  fn default() -> Self {
    Self::new()
  }
}

impl ExtensionInstaller for PersistencePluginProxyExtensionInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    register_serialization_registry_contributor(system, PersistenceSerializationContributor::new()).map_err(
      |error| ActorSystemBuildError::Configuration(format!("persistence serialization registration failed: {error}")),
    )?;
    let extension_id = PersistencePluginProxyExtensionId::new_with_settings(self.settings);
    install_extension_id(system, &extension_id);
    Ok(())
  }
}
