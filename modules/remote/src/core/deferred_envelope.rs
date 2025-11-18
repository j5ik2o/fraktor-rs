//! Placeholder for outbound envelopes queued until associations complete.

use alloc::string::String;

/// Represents a deferred outbound message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeferredEnvelope {
  tag: String,
}

impl DeferredEnvelope {
  /// Creates a new envelope tagged with the provided identifier.
  #[must_use]
  pub fn new(tag: impl Into<String>) -> Self {
    Self { tag: tag.into() }
  }

  /// Returns the tag (used for tests until actual envelope type is wired).
  #[must_use]
  pub fn tag(&self) -> &str {
    &self.tag
  }
}
