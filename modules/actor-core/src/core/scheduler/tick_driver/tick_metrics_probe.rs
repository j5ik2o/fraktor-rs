//! Metrics probe for scheduler tick feed.

#[cfg(test)]
mod tests;

use core::time::Duration;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, time::TimerInstant};

use super::{SchedulerTickMetrics, TickDriverKind, TickFeedHandle};

/// Captures scheduler tick metrics at configurable intervals.
pub struct SchedulerTickMetricsProbe<TB: RuntimeToolbox> {
  feed:       TickFeedHandle<TB>,
  resolution: Duration,
  driver:     TickDriverKind,
}

impl<TB: RuntimeToolbox> SchedulerTickMetricsProbe<TB> {
  /// Creates a new probe for the provided feed.
  #[must_use]
  pub const fn new(feed: TickFeedHandle<TB>, resolution: Duration, driver: TickDriverKind) -> Self {
    Self { feed, resolution, driver }
  }

  /// Collects a metrics snapshot at the specified instant.
  #[must_use]
  pub fn snapshot(&self, now: TimerInstant) -> SchedulerTickMetrics {
    self.feed.snapshot(now, self.driver)
  }

  /// Returns the associated driver classification.
  #[must_use]
  pub const fn driver(&self) -> TickDriverKind {
    self.driver
  }

  /// Returns the resolution used for expected tick calculations.
  #[must_use]
  pub const fn resolution(&self) -> Duration {
    self.resolution
  }
}
