//! Transport listener binding configuration.

use alloc::string::String;

/// Describes where the transport should listen for inbound associations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportBind {
  authority: String,
}

impl TransportBind {
  /// Creates a new binding for the specified authority (usually `host:port`).
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
