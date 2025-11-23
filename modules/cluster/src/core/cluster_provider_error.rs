//! Errors reported by cluster providers (no_std friendly).

extern crate alloc;

use alloc::string::String;

/// Error type returned by [`ClusterProvider`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterProviderError {
  /// Provider failed to start a member node.
  StartMemberFailed(String),
  /// Provider failed to start a client node.
  StartClientFailed(String),
  /// Provider shutdown failed.
  ShutdownFailed(String),
}

impl ClusterProviderError {
  /// Creates a start-member failure with the given reason.
  #[must_use]
  pub fn start_member(reason: impl Into<String>) -> Self {
    ClusterProviderError::StartMemberFailed(reason.into())
  }

  /// Creates a start-client failure with the given reason.
  #[must_use]
  pub fn start_client(reason: impl Into<String>) -> Self {
    ClusterProviderError::StartClientFailed(reason.into())
  }

  /// Creates a shutdown failure with the given reason.
  #[must_use]
  pub fn shutdown(reason: impl Into<String>) -> Self {
    ClusterProviderError::ShutdownFailed(reason.into())
  }

  /// Returns a human-readable reason string.
  #[must_use]
  pub const fn reason(&self) -> &str {
    match self {
      | ClusterProviderError::StartMemberFailed(reason)
      | ClusterProviderError::StartClientFailed(reason)
      | ClusterProviderError::ShutdownFailed(reason) => reason.as_str(),
    }
  }
}
