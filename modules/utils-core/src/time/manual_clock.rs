//! Manual deterministic clock implementation.

use core::time::Duration;

use super::{ClockKind, MonotonicClock, TimerInstant};

/// Deterministic clock advanced manually during tests.
#[derive(Clone, Copy, Debug)]
pub struct ManualClock {
  resolution: Duration,
  current:    TimerInstant,
}

impl ManualClock {
  /// Creates a clock anchored at zero.
  #[must_use]
  pub fn new(resolution: Duration) -> Self {
    Self { resolution, current: TimerInstant::zero(resolution) }
  }

  /// Advances the clock by the specified duration.
  pub fn advance(&mut self, duration: Duration) -> TimerInstant {
    let ticks = ticks_from_duration(duration, self.resolution);
    if ticks == 0 {
      return self.current;
    }
    self.current = self.current.saturating_add_ticks(ticks);
    self.current
  }
}

impl MonotonicClock for ManualClock {
  fn now(&self) -> TimerInstant {
    self.current
  }

  fn kind(&self) -> ClockKind {
    ClockKind::Deterministic
  }
}

fn ticks_from_duration(duration: Duration, resolution: Duration) -> u64 {
  if duration.is_zero() {
    return 0;
  }
  let resolution_ns = resolution.as_nanos().max(1);
  let duration_ns = duration.as_nanos();
  let mut ticks = duration_ns / resolution_ns;
  if ticks == 0 {
    ticks = 1;
  }
  u64::try_from(ticks).unwrap_or(u64::MAX)
}
