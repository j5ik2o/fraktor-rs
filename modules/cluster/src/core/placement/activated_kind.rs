//! Defines an activated cluster kind.

use alloc::string::String;

/// Describes a cluster kind that can be activated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivatedKind {
  name: String,
}

impl ActivatedKind {
  /// Creates a new activated kind with the provided name.
  #[must_use]
  pub fn new(name: impl Into<String>) -> Self {
    Self { name: name.into() }
  }

  /// Returns the kind name.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn name(&self) -> &str {
    &self.name
  }
}
