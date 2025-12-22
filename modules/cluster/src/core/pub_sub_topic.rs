//! Pub/Sub topic identifier.

use alloc::string::{String, ToString};

/// Pub/Sub topic name wrapper.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PubSubTopic(String);

impl PubSubTopic {
  /// Creates a new topic from the provided value.
  #[must_use]
  pub fn new(value: impl Into<String>) -> Self {
    Self(value.into())
  }

  /// Returns the topic as a string slice.
  #[must_use]
  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// Returns true if the topic is empty.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

impl From<String> for PubSubTopic {
  fn from(value: String) -> Self {
    Self(value)
  }
}

impl From<&str> for PubSubTopic {
  fn from(value: &str) -> Self {
    Self(value.to_string())
  }
}

impl core::fmt::Display for PubSubTopic {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{}", self.0)
  }
}
