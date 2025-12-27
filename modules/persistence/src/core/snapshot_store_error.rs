//! Snapshot store error types.

use alloc::string::String;
use core::fmt;

/// Errors returned by snapshot store operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotStoreError {
  /// Storage layer reported an error.
  Storage(String),
}

impl fmt::Display for SnapshotStoreError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | SnapshotStoreError::Storage(message) => write!(f, "snapshot store error: {message}"),
    }
  }
}
