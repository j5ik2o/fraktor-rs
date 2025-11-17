//! Transport scope metadata for serialization operations.

use alloc::string::String;

/// Carries contextual information about the transport performing serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportInformation {
  address: Option<String>,
}

impl TransportInformation {
  /// Creates a new instance from a textual address hint.
  #[must_use]
  pub const fn new(address: Option<String>) -> Self {
    Self { address }
  }

  /// Returns the optional address.
  #[must_use]
  pub fn address(&self) -> Option<&str> {
    self.address.as_deref()
  }
}
