//! Represents an outbound channel managed by a transport implementation.

use alloc::string::String;

/// Opaque identifier for an established outbound channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportChannel {
  authority: String,
}

impl TransportChannel {
  /// Creates a new channel descriptor for the authority.
  #[must_use]
  pub fn new(authority: impl Into<String>) -> Self {
    Self { authority: authority.into() }
  }

  /// Returns the authority associated with this channel.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }
}
