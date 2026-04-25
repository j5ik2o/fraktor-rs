//! Replacement payload for non-serializable throwable values.

use alloc::string::String;

/// Replacement payload used when an original throwable cannot be serialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThrowableNotSerializableException {
  original_message:    String,
  original_class_name: String,
}

impl ThrowableNotSerializableException {
  /// Creates a replacement payload for the original throwable.
  #[must_use]
  pub fn new(original_message: impl Into<String>, original_class_name: impl Into<String>) -> Self {
    Self { original_message: original_message.into(), original_class_name: original_class_name.into() }
  }

  /// Returns the original throwable message.
  #[must_use]
  pub const fn original_message(&self) -> &str {
    self.original_message.as_str()
  }

  /// Returns the original throwable class name.
  #[must_use]
  pub const fn original_class_name(&self) -> &str {
    self.original_class_name.as_str()
  }
}
