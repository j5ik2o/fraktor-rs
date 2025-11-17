//! Tracks actor names within a parent scope.

#[cfg(test)]
mod tests;

use alloc::{borrow::ToOwned, format, string::String};

use hashbrown::HashMap;

use crate::core::{actor_prim::Pid, spawn::NameRegistryError};

/// Maintains the mapping between actor names and their pids for a scope.
pub struct NameRegistry {
  entries: HashMap<String, Pid>,
}

impl Default for NameRegistry {
  fn default() -> Self {
    Self { entries: HashMap::new() }
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
    if self.entries.contains_key(name) {
      return Err(NameRegistryError::Duplicate(name.to_owned()));
    }
    self.entries.insert(name.to_owned(), pid);
    Ok(())
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
