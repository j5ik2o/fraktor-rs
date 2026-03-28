use alloc::{borrow::ToOwned, string::String};

use ahash::RandomState;
use hashbrown::{HashMap, hash_map::Entry};

use crate::core::kernel::dispatch::dispatcher::{DispatcherConfig, DispatcherRegistryError};

#[cfg(test)]
mod tests;

const DEFAULT_DISPATCHER_ID: &str = "default";
/// Reserved dispatcher id for blocking workloads (Pekko compatibility).
pub const DEFAULT_BLOCKING_DISPATCHER_ID: &str = "pekko.actor.default-blocking-io-dispatcher";

/// Registry that resolves dispatcher identifiers to configurations.
pub struct Dispatchers {
  entries: HashMap<String, DispatcherConfig, RandomState>,
}

impl Clone for Dispatchers {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone() }
  }
}

impl Dispatchers {
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
  pub fn register(&mut self, id: impl Into<String>, config: DispatcherConfig) -> Result<(), DispatcherRegistryError> {
    let id = id.into();
    match self.entries.entry(id) {
      | Entry::Occupied(entry) => Err(DispatcherRegistryError::duplicate(entry.key())),
      | Entry::Vacant(entry) => {
        entry.insert(config);
        Ok(())
      },
    }
  }

  /// Registers or updates a dispatcher configuration for the provided identifier.
  ///
  /// If the identifier already exists, the configuration is updated.
  pub fn register_or_update(&mut self, id: impl Into<String>, config: DispatcherConfig) {
    self.entries.insert(id.into(), config);
  }

  /// Resolves the dispatcher configuration for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatcherRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<DispatcherConfig, DispatcherRegistryError> {
    self.entries.get(id).cloned().ok_or_else(|| DispatcherRegistryError::unknown(id))
  }

  /// Ensures the default dispatcher entry exists.
  pub fn ensure_default(&mut self) {
    let default_config = self.entries.entry(DEFAULT_DISPATCHER_ID.to_owned()).or_default().clone();
    self.entries.entry(DEFAULT_BLOCKING_DISPATCHER_ID.to_owned()).or_insert(default_config);
  }
}

impl Default for Dispatchers {
  fn default() -> Self {
    Self::new()
  }
}
