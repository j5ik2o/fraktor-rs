//! Represents a listener registered with the transport.

use alloc::string::String;

/// Handle to a bound transport listener.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransportHandle {
  authority: String,
}

impl TransportHandle {
  /// Creates a handle referencing the provided authority.
  #[must_use]
  pub fn new(authority: impl Into<String>) -> Self {
    Self { authority: authority.into() }
  }

  /// Returns the bound authority.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }
}
