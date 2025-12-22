//! Errors returned by identity lookup operations.

use alloc::string::String;

#[cfg(test)]
mod tests;

/// Errors that can occur during identity lookup operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
  /// Coordinator is not ready to resolve.
  NotReady,
  /// No authority candidates were available.
  NoAuthority,
  /// Activation failed for the given grain key.
  ActivationFailed {
    /// Grain key that failed to activate.
    key: String,
  },
  /// Lookup is pending due to asynchronous processing.
  Pending,
  /// Lookup timed out.
  Timeout,
}
