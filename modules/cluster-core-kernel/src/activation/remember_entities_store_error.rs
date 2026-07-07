//! Errors returned by remembered entities store operations.

use alloc::string::String;
use core::{
  error::Error,
  fmt::{self, Formatter, Result as FmtResult},
};

#[cfg(test)]
#[path = "remember_entities_store_error_test.rs"]
mod tests;

/// Errors that can occur when interacting with a remembered entities store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RememberEntitiesStoreError {
  /// Entity identifier was invalid.
  InvalidEntityId {
    /// Invalid entity identifier.
    entity_id: String,
  },
  /// Entity was not found in the store.
  NotFound {
    /// Missing entity identifier.
    entity_id: String,
  },
  /// Store operation failed.
  Failed {
    /// Failure reason.
    reason: String,
  },
}

impl fmt::Display for RememberEntitiesStoreError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::InvalidEntityId { entity_id } => write!(f, "invalid entity id: {entity_id}"),
      | Self::NotFound { entity_id } => write!(f, "remembered entity not found: {entity_id}"),
      | Self::Failed { reason } => write!(f, "remember entities store failed: {reason}"),
    }
  }
}

impl Error for RememberEntitiesStoreError {}
