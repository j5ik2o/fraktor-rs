//! Errors returned by activation flow.

use alloc::string::String;

#[cfg(test)]
#[path = "activation_error_test.rs"]
mod tests;

/// Activation failure reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationError {
  /// Snapshot was required but missing.
  SnapshotMissing {
    /// Grain key string representation.
    key: String,
  },
  /// No authority candidates were provided.
  NoAuthority,
}
