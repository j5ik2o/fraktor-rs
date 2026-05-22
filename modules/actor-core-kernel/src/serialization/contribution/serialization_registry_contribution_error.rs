//! Serialization registry contribution error.

use core::fmt::{Display, Formatter, Result as FmtResult};

use crate::serialization::SerializationError;

/// Error returned while applying registry contributions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializationRegistryContributionError {
  message: SerializationError,
}

impl SerializationRegistryContributionError {
  /// Creates a new contribution error.
  #[must_use]
  pub const fn new(message: SerializationError) -> Self {
    Self { message }
  }

  /// Returns the underlying serialization error.
  #[must_use]
  pub const fn message(&self) -> &SerializationError {
    &self.message
  }
}

impl Display for SerializationRegistryContributionError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
    write!(formatter, "serialization registry contribution failed: {:?}", self.message)
  }
}
