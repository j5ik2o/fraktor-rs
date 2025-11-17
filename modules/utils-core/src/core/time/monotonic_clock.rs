//! Monotonic clock abstraction.

use super::{ClockKind, TimerInstant};

/// Monotonic clock abstraction shared across runtimes.
pub trait MonotonicClock: Send + Sync + 'static {
  /// Returns the latest monotonic instant.
  fn now(&self) -> TimerInstant;

  /// Identifies the clock flavor.
  fn kind(&self) -> ClockKind;
}
