//! Remembered entities store port.

use alloc::{collections::BTreeSet, string::String};

use super::RememberEntitiesStoreError;

#[cfg(test)]
#[path = "remember_entities_store_test.rs"]
mod tests;

/// Port for persisting entity identifiers across shard restarts and rebalances.
pub trait RememberEntitiesStore {
  /// Lists all remembered entity identifiers.
  ///
  /// # Errors
  ///
  /// Returns [`RememberEntitiesStoreError`] when the store cannot be read.
  fn list_entities(&self) -> Result<BTreeSet<String>, RememberEntitiesStoreError>;

  /// Adds one remembered entity identifier.
  ///
  /// # Errors
  ///
  /// Returns [`RememberEntitiesStoreError`] when the entity cannot be stored.
  fn add_entity(&mut self, entity_id: String) -> Result<(), RememberEntitiesStoreError>;

  /// Removes one remembered entity identifier.
  ///
  /// # Errors
  ///
  /// Returns [`RememberEntitiesStoreError`] when the entity cannot be removed.
  fn remove_entity(&mut self, entity_id: String) -> Result<(), RememberEntitiesStoreError>;
}
