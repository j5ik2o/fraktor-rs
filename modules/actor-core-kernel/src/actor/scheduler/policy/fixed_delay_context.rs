//! State machine governing fixed-delay periodic jobs.

use core::num::{NonZeroU32, NonZeroU64};

use super::periodic_batch_decision::PeriodicBatchDecision;
use crate::actor::scheduler::{BatchMode, ExecutionBatch, SchedulerWarning};

/// Tracks timing metadata for fixed-delay jobs.
pub(crate) struct FixedDelayContext {
  next_tick:       u64,
  period_ticks:    NonZeroU64,
  backlog_limit:   NonZeroU32,
  burst_threshold: NonZeroU32,
  skip_missed:     bool,
}

impl FixedDelayContext {
  /// Creates a new context starting at the provided tick.
  pub(crate) const fn new(
    start_tick: u64,
    period_ticks: NonZeroU64,
    backlog_limit: NonZeroU32,
    burst_threshold: NonZeroU32,
  ) -> Self {
    Self { next_tick: start_tick, period_ticks, backlog_limit, burst_threshold, skip_missed: false }
  }

  pub(crate) const fn new_skipping_missed(
    start_tick: u64,
    period_ticks: NonZeroU64,
    backlog_limit: NonZeroU32,
    burst_threshold: NonZeroU32,
  ) -> Self {
    Self { next_tick: start_tick, period_ticks, backlog_limit, burst_threshold, skip_missed: true }
  }

  pub(crate) fn build_batch(&mut self, now: u64, handle_id: u64) -> PeriodicBatchDecision {
    let missed = self.compute_missed(now);
    if !self.skip_missed && missed >= self.backlog_limit.get() {
      return PeriodicBatchDecision::Cancel { warning: SchedulerWarning::BacklogExceeded { handle_id, missed } };
    }

    let warning = if !self.skip_missed && missed > self.burst_threshold.get() {
      Some(SchedulerWarning::BurstFire { handle_id, missed })
    } else {
      None
    };

    let runs_total = if self.skip_missed { 1 } else { missed.saturating_add(1) };
    // SAFETY: runs_total is at least 1 (0 + 1)
    let runs = unsafe { NonZeroU32::new_unchecked(runs_total) };
    self.next_tick = now.saturating_add(self.period_ticks.get());
    PeriodicBatchDecision::Execute { batch: ExecutionBatch::periodic(runs, missed, BatchMode::FixedDelay), warning }
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
