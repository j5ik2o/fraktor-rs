//! Grain key used for virtual actor identification.

use alloc::string::String;

#[cfg(test)]
mod tests;

/// Immutable grain key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GrainKey {
  value: String,
}

impl GrainKey {
  /// Creates a new grain key.
  #[must_use]
  pub const fn new(value: String) -> Self {
    Self { value }
  }

  /// Returns the underlying string.
  #[must_use]
  pub fn value(&self) -> &str {
    &self.value
  }
}
