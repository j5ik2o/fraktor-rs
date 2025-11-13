//! Handle returned when scheduling jobs.

/// Identifier for scheduled jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SchedulerHandle(u64);

impl SchedulerHandle {
  /// Creates a new handle from raw identifier.
  #[must_use]
  pub const fn new(raw: u64) -> Self {
    Self(raw)
  }

  /// Returns the inner identifier.
  #[must_use]
  pub const fn raw(&self) -> u64 {
    self.0
  }
}
