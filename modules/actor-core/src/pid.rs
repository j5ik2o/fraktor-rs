//! Actor process identifier utilities.

use core::fmt;

/// Unique process identifier assigned to each actor cell.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Pid {
  value:      u64,
  generation: u32,
}

impl Pid {
  /// Creates a new PID from the provided numeric components.
  #[must_use]
  pub const fn new(value: u64, generation: u32) -> Self {
    Self { value, generation }
  }

  /// Returns the numeric value of the PID.
  #[must_use]
  pub const fn value(&self) -> u64 {
    self.value
  }

  /// Returns the generation associated with the PID.
  #[must_use]
  pub const fn generation(&self) -> u32 {
    self.generation
  }

  /// Computes a stable registry key used for constant-time lookups.
  #[must_use]
  pub const fn registry_key(&self) -> u128 {
    (self.generation as u128) << 64 | self.value as u128
  }
}

impl fmt::Debug for Pid {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Pid({}, gen:{})", self.value, self.generation)
  }
}
