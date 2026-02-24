//! Event emitted when the current actor system has been quarantined by a remote peer.

use alloc::string::String;

/// Lifecycle payload describing that this actor system has been quarantined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThisActorSystemQuarantinedEvent {
  local_authority:  String,
  remote_authority: String,
}

impl ThisActorSystemQuarantinedEvent {
  /// Creates a new payload.
  #[must_use]
  pub fn new(local_authority: impl Into<String>, remote_authority: impl Into<String>) -> Self {
    Self { local_authority: local_authority.into(), remote_authority: remote_authority.into() }
  }

  /// Returns the canonical authority of this actor system.
  #[must_use]
  pub fn local_authority(&self) -> &str {
    &self.local_authority
  }

  /// Returns the authority that quarantined this actor system.
  #[must_use]
  pub fn remote_authority(&self) -> &str {
    &self.remote_authority
  }
}
