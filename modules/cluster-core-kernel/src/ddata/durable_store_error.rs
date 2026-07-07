//! Errors returned by durable store operations.

use alloc::string::String;

/// Errors that can occur when interacting with a durable store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurableStoreError {
  /// Startup load failed and must abort replicator initialization.
  LoadFailed {
    /// Failure reason.
    reason: String,
  },
  /// Persisting one entry failed.
  StoreFailed {
    /// Failure reason.
    reason: String,
  },
}
