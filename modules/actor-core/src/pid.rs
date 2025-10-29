//! Process identifier implementation.

/// Identifies an actor instance within the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pid {
  value:      u64,
  generation: u32,
}

impl Pid {
  /// Creates a new PID using the provided identifier and generation.
  #[must_use]
  pub const fn new(value: u64, generation: u32) -> Self {
    Self { value, generation }
  }

  /// Returns the numeric identifier component.
  #[must_use]
  pub const fn value(&self) -> u64 {
    self.value
  }

  /// Returns the generation counter ensuring uniqueness when IDs are recycled.
  #[must_use]
  pub const fn generation(&self) -> u32 {
    self.generation
  }

  /// Creates a PID representing the next generation for the same numeric identifier.
  #[must_use]
  pub const fn next_generation(&self) -> Self {
    Self { value: self.value, generation: self.generation.wrapping_add(1) }
  }
}

impl Default for Pid {
  fn default() -> Self {
    Self::new(0, 0)
  }
}
