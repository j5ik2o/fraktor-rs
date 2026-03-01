//! Stream attributes used to annotate stages and graphs.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

/// Immutable collection of stream attributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attributes {
  names: Vec<String>,
}

impl Attributes {
  /// Creates an empty attributes collection.
  #[must_use]
  pub const fn new() -> Self {
    Self { names: Vec::new() }
  }

  /// Creates attributes containing a single stage name.
  #[must_use]
  pub fn named(name: impl Into<String>) -> Self {
    Self { names: alloc::vec![name.into()] }
  }

  /// Appends names from another attributes collection and returns a new value.
  #[must_use]
  pub fn and(mut self, other: Self) -> Self {
    self.names.extend(other.names);
    self
  }

  /// Returns all configured stage names.
  #[must_use]
  pub fn names(&self) -> &[String] {
    &self.names
  }

  /// Returns `true` when no attributes have been configured.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.names.is_empty()
  }
}

impl Default for Attributes {
  fn default() -> Self {
    Self::new()
  }
}
