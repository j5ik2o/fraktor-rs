//! Errors returned by activation storage operations.

use alloc::string::String;

/// Errors that can occur when interacting with activation storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationStorageError {
  /// Storage operation failed.
  Failed {
    /// Failure reason.
    reason: String,
  },
}
