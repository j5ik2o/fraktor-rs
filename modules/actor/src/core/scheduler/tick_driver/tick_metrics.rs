//! Scheduler tick metrics snapshot state.

use core::time::Duration;

use super::TickDriverKind;

/// Aggregated tick statistics published through EventStream.
#[derive(Clone, Debug)]
pub struct SchedulerTickMetrics {
  driver:         TickDriverKind,
  ticks_per_sec:  u32,
  drift:          Option<Duration>,
  enqueued_total: u64,
  dropped_total:  u64,
}

impl SchedulerTickMetrics {
  /// Creates a metrics snapshot.
  #[allow(dead_code)]
  pub(crate) const fn new(
    driver: TickDriverKind,
    ticks_per_sec: u32,
    drift: Option<Duration>,
    enqueued_total: u64,
    dropped_total: u64,
  ) -> Self {
    Self { driver, ticks_per_sec, drift, enqueued_total, dropped_total }
  }

  /// Driver classification that produced this snapshot.
  #[must_use]
  pub const fn driver(&self) -> TickDriverKind {
    self.driver
  }

  /// Measured ticks per second.
  #[must_use]
  pub const fn ticks_per_sec(&self) -> u32 {
    self.ticks_per_sec
  }

  /// Observed drift relative to configured resolution.
  #[must_use]
  pub const fn drift(&self) -> Option<Duration> {
    self.drift
  }

  /// Total enqueued ticks since driver start.
  #[must_use]
  pub const fn enqueued_total(&self) -> u64 {
    self.enqueued_total
  }

  /// Total dropped ticks.
  #[must_use]
  pub const fn dropped_total(&self) -> u64 {
    self.dropped_total
  }
}
