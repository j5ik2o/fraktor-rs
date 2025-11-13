//! State machine governing fixed-rate periodic jobs.

use core::num::{NonZeroU32, NonZeroU64};

use super::{
  execution_batch::{BatchMode, ExecutionBatch},
  periodic_batch_decision::PeriodicBatchDecision,
  warning::SchedulerWarning,
};

/// Tracks timing metadata for fixed-rate jobs.
pub(crate) struct FixedRateContext {
  next_tick:       u64,
  period_ticks:    NonZeroU64,
  backlog_limit:   NonZeroU32,
  burst_threshold: NonZeroU32,
}

impl FixedRateContext {
  /// Creates a new context starting at the provided tick.
  pub(crate) const fn new(
    start_tick: u64,
    period_ticks: NonZeroU64,
    backlog_limit: NonZeroU32,
    burst_threshold: NonZeroU32,
  ) -> Self {
    Self { next_tick: start_tick, period_ticks, backlog_limit, burst_threshold }
  }

  pub(crate) fn build_batch(&mut self, now: u64, handle_id: u64) -> PeriodicBatchDecision {
    let missed = self.compute_missed(now);
    if missed >= self.backlog_limit.get() {
      return PeriodicBatchDecision::Cancel { warning: SchedulerWarning::BacklogExceeded { handle_id, missed } };
    }

    let warning =
      if missed > self.burst_threshold.get() { Some(SchedulerWarning::BurstFire { handle_id, missed }) } else { None };

    let runs_total = missed.saturating_add(1);
    let runs = NonZeroU32::new(runs_total).expect("non-zero runs");
    self.next_tick = self.next_tick.saturating_add(self.period_ticks.get().saturating_mul(u64::from(runs_total)));
    PeriodicBatchDecision::Execute { batch: ExecutionBatch::periodic(runs, missed, BatchMode::FixedRate), warning }
  }

  pub(crate) const fn next_deadline_ticks(&self) -> u64 {
    self.next_tick
  }

  fn compute_missed(&self, now: u64) -> u32 {
    if now <= self.next_tick {
      return 0;
    }
    let delta = now - self.next_tick;
    let period = self.period_ticks.get();
    let raw = delta / period;
    raw.min(u32::MAX as u64) as u32
  }
}
