//! Persistence identity.

use alloc::{format, string::String};

/// Identifies one persistent event stream.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersistenceId {
  value: String,
}

impl PersistenceId {
  /// Creates a persistence id from an already unique value.
  #[must_use]
  pub fn of_unique_id(value: impl Into<String>) -> Self {
    Self { value: value.into() }
  }

  /// Creates a persistence id from an entity type hint and entity id.
  #[must_use]
  pub fn of_entity_id(entity_type_hint: impl AsRef<str>, entity_id: impl AsRef<str>) -> Self {
    Self { value: format!("{}|{}", entity_type_hint.as_ref(), entity_id.as_ref()) }
  }

  /// Returns the persistence id as a string slice.
  #[must_use]
  pub fn as_str(&self) -> &str {
    self.value.as_str()
  }
}
