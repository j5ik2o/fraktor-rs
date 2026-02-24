//! Event emitted when a remote authority is quarantined due to graceful shutdown.

use alloc::string::String;

/// Lifecycle payload describing a graceful-shutdown quarantine transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GracefulShutdownQuarantinedEvent {
  authority: String,
  uid:       u64,
  reason:    String,
}

impl GracefulShutdownQuarantinedEvent {
  /// Creates a new graceful-shutdown quarantine event payload.
  #[must_use]
  pub fn new(authority: impl Into<String>, uid: u64, reason: impl Into<String>) -> Self {
    Self { authority: authority.into(), uid, reason: reason.into() }
  }

  /// Returns the quarantined authority identifier.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the remote actor-system UID.
  #[must_use]
  pub const fn uid(&self) -> u64 {
    self.uid
  }

  /// Returns the human-readable quarantine reason.
  #[must_use]
  pub fn reason(&self) -> &str {
    &self.reason
  }
}
