//! Unique identifier assigned to tick drivers.

/// Unique identifier for a tick driver instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TickDriverId(u64);

impl TickDriverId {
  /// Creates a new tick driver ID.
  #[must_use]
  pub const fn new(id: u64) -> Self {
    Self(id)
  }

  /// Returns the underlying ID value.
  #[must_use]
  pub const fn as_u64(self) -> u64 {
    self.0
  }
}
