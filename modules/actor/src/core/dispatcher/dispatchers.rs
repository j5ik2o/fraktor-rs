use alloc::{borrow::ToOwned, string::String};

use ahash::RandomState;
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};
use hashbrown::HashMap;

use crate::core::dispatcher::{DispatcherConfigGeneric, DispatcherRegistryError};

#[cfg(test)]
mod tests;

const DEFAULT_DISPATCHER_ID: &str = "default";

/// Registry that resolves dispatcher identifiers to configurations.
pub struct DispatchersGeneric<TB: RuntimeToolbox + 'static> {
  entries: HashMap<String, DispatcherConfigGeneric<TB>, RandomState>,
}

/// Type alias using the default toolbox.
pub type Dispatchers = DispatchersGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> DispatchersGeneric<TB> {
  /// Creates an empty dispatcher registry.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()) }
  }

  /// Registers a dispatcher configuration for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatcherRegistryError::Duplicate`] when the identifier already exists.
  pub fn register(
    &mut self,
    id: impl Into<String>,
    config: DispatcherConfigGeneric<TB>,
  ) -> Result<(), DispatcherRegistryError> {
    let id = id.into();
    if self.entries.contains_key(&id) {
      return Err(DispatcherRegistryError::duplicate(&id));
    }
    self.entries.insert(id, config);
    Ok(())
  }

  /// Registers or updates a dispatcher configuration for the provided identifier.
  ///
  /// If the identifier already exists, the configuration is updated.
  pub fn register_or_update(&mut self, id: impl Into<String>, config: DispatcherConfigGeneric<TB>) {
    self.entries.insert(id.into(), config);
  }

  /// Resolves the dispatcher configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatcherRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<DispatcherConfigGeneric<TB>, DispatcherRegistryError> {
    self.entries.get(id).cloned().ok_or_else(|| DispatcherRegistryError::unknown(id))
  }

  /// Ensures the default dispatcher entry exists.
  pub fn ensure_default(&mut self) {
    self.entries.entry(DEFAULT_DISPATCHER_ID.to_owned()).or_default();
  }
}

impl<TB: RuntimeToolbox + 'static> Default for DispatchersGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
