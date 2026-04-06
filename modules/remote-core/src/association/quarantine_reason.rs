//! Human-readable reason attached to a quarantine transition.

use alloc::string::String;

/// Human-readable description of why a remote peer was quarantined.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QuarantineReason {
  message: String,
}

impl QuarantineReason {
  /// Creates a new [`QuarantineReason`] from the supplied message.
  #[must_use]
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: message.into() }
  }

  /// Returns the stored message text.
  #[must_use]
  pub fn message(&self) -> &str {
    &self.message
  }
}
