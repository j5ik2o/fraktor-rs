//! Errors returned by placement lock operations.

use alloc::string::String;

/// Errors that can occur when acquiring or releasing locks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementLockError {
  /// Lock acquisition or release failed.
  Failed {
    /// Failure reason.
    reason: String,
  },
}
