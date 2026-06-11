//! Extension identifier for persistence plugin proxy actors.

use fraktor_actor_core_kernel_rs::{actor::extension::ExtensionId, system::ActorSystem};

use crate::{
  config::PersistenceConfig,
  extension::{PersistenceExtension, PersistenceExtensionShared},
};

/// Registers and instantiates a persistence extension backed by proxy actors.
pub struct PersistencePluginProxyExtensionId {
  settings: PersistenceConfig,
}

impl PersistencePluginProxyExtensionId {
  /// Creates a new proxy extension identifier.
  #[must_use]
  pub const fn new() -> Self {
    Self::new_with_settings(PersistenceConfig::default_config())
  }

  /// Creates a new proxy extension identifier with explicit settings.
  #[must_use]
  pub const fn new_with_settings(settings: PersistenceConfig) -> Self {
    Self { settings }
  }
}

impl Default for PersistencePluginProxyExtensionId {
  fn default() -> Self {
    Self::new()
  }
}

impl ExtensionId for PersistencePluginProxyExtensionId {
  type Ext = PersistenceExtensionShared;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    let extension = match PersistenceExtension::new_proxy_with_settings(system, self.settings) {
      | Ok(extension) => extension,
      | Err(error) => {
        panic!("persistence plugin proxy extension bootstrap failed: {error:?}");
      },
    };
    PersistenceExtensionShared::new(extension)
  }
}
