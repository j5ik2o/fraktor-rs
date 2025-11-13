//! Scheduler metrics exposed for observability and tests.

/// Minimal metrics recorded by the scheduler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SchedulerMetrics {
  active_timers: usize,
  dropped_total: usize,
}

impl SchedulerMetrics {
  /// Active timers awaiting execution.
  #[must_use]
  pub const fn active_timers(&self) -> usize {
    self.active_timers
  }

  /// Count of timers dropped before execution (e.g., cancellation races).
  #[must_use]
  pub const fn dropped_total(&self) -> usize {
    self.dropped_total
  }

  pub(crate) fn increment_active(&mut self) {
    self.active_timers = self.active_timers.saturating_add(1);
  }

  pub(crate) fn decrement_active(&mut self) {
    if self.active_timers > 0 {
      self.active_timers -= 1;
    }
  }

  pub(crate) fn increment_dropped(&mut self) {
    self.dropped_total = self.dropped_total.saturating_add(1);
  }
}
