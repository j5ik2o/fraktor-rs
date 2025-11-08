//! Identifier for context pipe tasks scheduled within an actor cell.

use core::fmt;

/// Unique identifier assigned to each pipe task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContextPipeTaskId(u64);

impl ContextPipeTaskId {
  /// Creates a new identifier from the provided numeric value.
  #[must_use]
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  /// Returns the raw numeric value of the identifier.
  #[must_use]
  pub const fn get(&self) -> u64 {
    self.0
  }
}

impl fmt::Display for ContextPipeTaskId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}
