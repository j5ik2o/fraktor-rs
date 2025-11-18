//! Describes a remote authority targeted by a channel.

use alloc::string::String;

/// Remote authority descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportEndpoint {
  authority: String,
}

impl TransportEndpoint {
  /// Creates a new endpoint by authority string.
  #[must_use]
  pub fn new(authority: String) -> Self {
    Self { authority }
  }

  /// Returns the authority string.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }
}
