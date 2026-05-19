//! Monotonic tick instant utilities.

use core::time::Duration;

/// Monotonic instant with fixed resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TimerInstant {
  ticks:      u64,
  resolution: Duration,
}

impl TimerInstant {
  /// Creates an instant anchored at zero.
  #[must_use]
  pub const fn zero(resolution: Duration) -> Self {
    Self { ticks: 0, resolution }
  }

  /// Creates an instant from raw tick count and resolution.
  #[must_use]
  pub const fn from_ticks(ticks: u64, resolution: Duration) -> Self {
    Self { ticks, resolution }
  }

  /// Returns the stored tick count.
  #[must_use]
  pub const fn ticks(&self) -> u64 {
    self.ticks
  }

  /// Returns the resolution of each tick.
  #[must_use]
  pub const fn resolution(&self) -> Duration {
    self.resolution
  }

  /// Adds ticks, saturating on overflow.
  #[must_use]
  pub const fn saturating_add_ticks(&self, ticks: u64) -> Self {
    Self { ticks: self.ticks.saturating_add(ticks), resolution: self.resolution }
  }
}
