//! In-memory remembered entities store.

use alloc::{collections::BTreeSet, string::String};

use super::{RememberEntitiesStore, RememberEntitiesStoreError};

#[cfg(test)]
#[path = "in_memory_remember_entities_store_test.rs"]
mod tests;

/// In-memory implementation of [`RememberEntitiesStore`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InMemoryRememberEntitiesStore {
  entities: BTreeSet<String>,
}

impl InMemoryRememberEntitiesStore {
  /// Creates an empty store.
  #[must_use]
  pub const fn new() -> Self {
    Self { entities: BTreeSet::new() }
  }

  const fn validate_entity_id(entity_id: &str) -> Result<(), RememberEntitiesStoreError> {
    if entity_id.is_empty() {
      return Err(RememberEntitiesStoreError::InvalidEntityId { entity_id: String::new() });
    }
    Ok(())
  }
}

impl RememberEntitiesStore for InMemoryRememberEntitiesStore {
  fn list_entities(&self) -> Result<BTreeSet<String>, RememberEntitiesStoreError> {
    Ok(self.entities.clone())
  }

  fn add_entity(&mut self, entity_id: String) -> Result<(), RememberEntitiesStoreError> {
    Self::validate_entity_id(&entity_id)?;
    self.entities.insert(entity_id);
    Ok(())
  }

  fn remove_entity(&mut self, entity_id: String) -> Result<(), RememberEntitiesStoreError> {
    Self::validate_entity_id(&entity_id)?;
    if self.entities.remove(&entity_id) { Ok(()) } else { Err(RememberEntitiesStoreError::NotFound { entity_id }) }
  }
}
