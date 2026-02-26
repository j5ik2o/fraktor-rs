//! Tracks actor names within a parent scope.

#[cfg(test)]
mod tests;

use alloc::{borrow::ToOwned, format, string::String};

use ahash::RandomState;
use hashbrown::{HashMap, hash_map::Entry};

use crate::core::{actor::Pid, spawn::NameRegistryError};

/// Maintains the mapping between actor names and their pids for a scope.
pub struct NameRegistry {
  entries: HashMap<String, Pid, RandomState>,
}

impl Default for NameRegistry {
  fn default() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()) }
  }
}

impl NameRegistry {
  /// Creates a new, empty registry.
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Attempts to register a name for the provided pid.
  ///
  /// # Errors
  ///
  /// Returns an error if the name is already registered in this scope.
  pub fn register(&mut self, name: &str, pid: Pid) -> Result<(), NameRegistryError> {
    match self.entries.entry(name.to_owned()) {
      | Entry::Occupied(entry) => Err(NameRegistryError::Duplicate(entry.key().clone())),
      | Entry::Vacant(entry) => {
        entry.insert(pid);
        Ok(())
      },
    }
  }

  /// Resolves a name to the stored pid if present.
  #[must_use]
  pub fn resolve(&self, name: &str) -> Option<Pid> {
    self.entries.get(name).copied()
  }

  /// Removes the provided name from the registry and returns the previous pid.
  pub fn remove(&mut self, name: &str) -> Option<Pid> {
    self.entries.remove(name)
  }

  /// Generates an anonymous fallback name derived from the pid.
  #[must_use]
  pub fn generate_anonymous(&self, pid: Pid) -> String {
    format!("anon-{}", pid)
  }
}
