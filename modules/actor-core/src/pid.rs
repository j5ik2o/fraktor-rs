//! Unique actor identifiers.

use core::fmt;

/// Process identifier allocated by the actor system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pid {
  value:      u64,
  generation: u32,
}

impl Pid {
  /// Creates a new process identifier.
  #[must_use]
  pub const fn new(value: u64, generation: u32) -> Self {
    Self { value, generation }
  }

  /// Returns the numeric value assigned to the pid.
  #[must_use]
  pub const fn value(&self) -> u64 {
    self.value
  }

  /// Returns the generation counter.
  #[must_use]
  pub const fn generation(&self) -> u32 {
    self.generation
  }
}

impl fmt::Display for Pid {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}:{}", self.value, self.generation)
  }
}
