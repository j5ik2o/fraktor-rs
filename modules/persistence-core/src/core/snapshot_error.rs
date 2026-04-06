//! Snapshot store operation errors.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::fmt;

/// Errors returned by snapshot store operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotError {
  /// Failed to save a snapshot.
  SaveFailed(String),
  /// Failed to load a snapshot.
  LoadFailed(String),
  /// Failed to delete a snapshot.
  DeleteFailed(String),
}

impl fmt::Display for SnapshotError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | SnapshotError::SaveFailed(reason) => write!(formatter, "save snapshot failed: {}", reason),
      | SnapshotError::LoadFailed(reason) => write!(formatter, "load snapshot failed: {}", reason),
      | SnapshotError::DeleteFailed(reason) => write!(formatter, "delete snapshot failed: {}", reason),
    }
  }
}
