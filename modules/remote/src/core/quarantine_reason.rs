//! Describes why an authority entered the quarantined state.

use alloc::string::String;

/// Human-readable explanation for quarantine transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuarantineReason {
  message: String,
}

impl QuarantineReason {
  /// Creates a new reason with the provided message text.
  #[must_use]
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: message.into() }
  }

  /// Returns the stored message.
  #[must_use]
  pub fn message(&self) -> &str {
    &self.message
  }
}
