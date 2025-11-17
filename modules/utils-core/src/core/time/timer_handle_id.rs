//! Identifier assigned to timer entries.

/// Identifier assigned to scheduled timer entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct TimerHandleId(u64);

impl TimerHandleId {
  /// Creates a new identifier.
  #[must_use]
  pub const fn new(raw: u64) -> Self {
    Self(raw)
  }

  /// Returns the raw value.
  #[must_use]
  pub const fn raw(&self) -> u64 {
    self.0
  }
}
