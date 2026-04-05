use alloc::{borrow::ToOwned, string::String};

use ahash::RandomState;
use hashbrown::{HashMap, hash_map::Entry};

use crate::core::kernel::dispatch::dispatcher::{
  DispatcherRegistryEntry, DispatcherRegistryError, DispatcherSettings, InlineDispatcherProvider,
};

#[cfg(test)]
mod tests;

/// Reserved kernel registry identifier for the default dispatcher entry.
pub const DEFAULT_DISPATCHER_ID: &str = "default";
const PEKKO_DEFAULT_DISPATCHER_ID: &str = "pekko.actor.default-dispatcher";
const PEKKO_INTERNAL_DISPATCHER_ID: &str = "pekko.actor.internal-dispatcher";
/// Reserved dispatcher id for blocking workloads (Pekko compatibility).
pub const DEFAULT_BLOCKING_DISPATCHER_ID: &str = "pekko.actor.default-blocking-io-dispatcher";

/// Registry that resolves dispatcher identifiers to provider/settings entries.
pub struct Dispatchers {
  entries: HashMap<String, DispatcherRegistryEntry, RandomState>,
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

  /// Registers a dispatcher registry entry for the provided identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatcherRegistryError::Duplicate`] when the identifier already exists.
  pub fn register(
    &mut self,
    id: impl Into<String>,
    entry: DispatcherRegistryEntry,
  ) -> Result<(), DispatcherRegistryError> {
    let id = id.into();
    match self.entries.entry(id) {
      | Entry::Occupied(existing) => Err(DispatcherRegistryError::duplicate(existing.key())),
      | Entry::Vacant(vacant) => {
        vacant.insert(entry);
        Ok(())
      },
    }
  }

  /// Registers or updates a dispatcher registry entry for the provided identifier.
  ///
  /// If the identifier already exists, the configuration is updated.
  pub fn register_or_update(&mut self, id: impl Into<String>, entry: DispatcherRegistryEntry) {
    self.entries.insert(id.into(), entry);
  }

  /// Resolves the dispatcher registry entry for the identifier.
  ///
  /// # Errors
  ///
  /// Returns [`DispatcherRegistryError::Unknown`] when the identifier has not been registered.
  pub fn resolve(&self, id: &str) -> Result<DispatcherRegistryEntry, DispatcherRegistryError> {
    let id = Self::normalize_dispatcher_id(id);
    self.entries.get(id).cloned().ok_or_else(|| DispatcherRegistryError::unknown(id))
  }

  /// Ensures the default dispatcher entry exists.
  pub fn ensure_default(&mut self) {
    let default_entry = self
      .entries
      .entry(DEFAULT_DISPATCHER_ID.to_owned())
      .or_insert_with(|| DispatcherRegistryEntry::new(InlineDispatcherProvider::new(), DispatcherSettings::default()))
      .clone();
    self.entries.entry(DEFAULT_BLOCKING_DISPATCHER_ID.to_owned()).or_insert(default_entry);
  }

  pub(crate) fn normalize_dispatcher_id(id: &str) -> &str {
    match id {
      | PEKKO_DEFAULT_DISPATCHER_ID | PEKKO_INTERNAL_DISPATCHER_ID => DEFAULT_DISPATCHER_ID,
      | _ => id,
    }
  }
}

impl Default for Dispatchers {
  fn default() -> Self {
    Self::new()
  }
}
