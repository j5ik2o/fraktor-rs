//! Grain key used for virtual actor identification.

use alloc::string::String;

/// Immutable grain key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GrainKey {
  value: String,
}

impl GrainKey {
  /// Creates a new grain key.
  pub fn new(value: String) -> Self {
    Self { value }
  }

  /// Returns the underlying string.
  pub fn value(&self) -> &str {
    &self.value
  }
}

#[cfg(test)]
mod tests;
