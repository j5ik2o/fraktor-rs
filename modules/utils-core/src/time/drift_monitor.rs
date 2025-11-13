//! Drift monitoring helpers for the scheduler.

use core::time::Duration;

use super::{DriftStatus, TimerInstant, TimerWheelConfig};

/// Monitors clock drift and reports warnings once the budget is exceeded.
pub struct DriftMonitor {
  config:        TimerWheelConfig,
  last_deadline: TimerInstant,
  exceeded:      Option<Duration>,
}

impl DriftMonitor {
  /// Creates a monitor anchored at the provided deadline.
  #[must_use]
  pub const fn new(config: TimerWheelConfig, anchor: TimerInstant) -> Self {
    Self { config, last_deadline: anchor, exceeded: None }
  }

  /// Records an observation comparing deadline vs actual clock.
  pub fn observe(&mut self, deadline: TimerInstant, actual: TimerInstant) -> DriftStatus {
    self.last_deadline = deadline;
    let resolution_ns = self.config.resolution().as_nanos().max(1);
    let tick_diff = actual.ticks().abs_diff(deadline.ticks());
    let drift_ns = (tick_diff as u128).saturating_mul(resolution_ns);
    let budget_ns =
      (resolution_ns.saturating_mul(self.config.drift_budget_pct() as u128)).checked_div(100).unwrap_or(0);

    if drift_ns > budget_ns {
      let clamped = drift_ns.min(u128::from(u64::MAX));
      let observed = Duration::from_nanos(clamped as u64);
      self.exceeded = Some(observed);
      DriftStatus::Exceeded { observed }
    } else {
      self.exceeded = None;
      DriftStatus::WithinBudget
    }
  }

  /// Returns the last recorded drift amount when exceeded.
  #[must_use]
  pub const fn last_exceeded(&self) -> Option<Duration> {
    self.exceeded
  }
}
