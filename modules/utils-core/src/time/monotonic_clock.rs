//! Monotonic clock abstraction.

use super::TimerInstant;

/// Monotonic clock abstraction shared across runtimes.
pub trait MonotonicClock: Send + Sync + 'static {
  /// Returns the latest monotonic instant.
  fn now(&self) -> TimerInstant;
}
