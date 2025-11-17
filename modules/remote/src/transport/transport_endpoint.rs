//! Remote transport endpoint descriptor.

use alloc::string::String;

/// Describes a remote authority that the transport can dial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportEndpoint {
  authority: String,
}

impl TransportEndpoint {
  /// Creates a new endpoint descriptor.
  #[must_use]
  pub fn new(authority: impl Into<String>) -> Self {
    Self { authority: authority.into() }
  }

  /// Returns the authority identifier.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }
}
