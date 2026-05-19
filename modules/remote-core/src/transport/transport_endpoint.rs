//! Remote authority descriptor targeted by a transport channel.

use alloc::string::String;

/// Remote authority descriptor used by a transport channel.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransportEndpoint {
  authority: String,
}

impl TransportEndpoint {
  /// Creates a new endpoint from an authority string.
  #[must_use]
  pub fn new(authority: impl Into<String>) -> Self {
    Self { authority: authority.into() }
  }

  /// Returns the authority string.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }
}
