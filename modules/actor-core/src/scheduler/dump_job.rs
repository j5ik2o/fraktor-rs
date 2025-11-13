//! Metadata describing scheduled jobs in diagnostic dumps.

use super::mode::SchedulerMode;

/// Metadata describing a scheduled job inside the dump snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerDumpJob {
  /// Handle identifier for the job.
  handle_id:     u64,
  /// Scheduling mode (one-shot, fixed-rate, fixed-delay).
  mode:          SchedulerMode,
  /// Deadline tick recorded when the dump was produced.
  deadline_tick: u64,
  /// Next periodic tick when available.
  next_tick:     Option<u64>,
}

impl SchedulerDumpJob {
  /// Creates a new dump job entry.
  #[must_use]
  pub const fn new(handle_id: u64, mode: SchedulerMode, deadline_tick: u64, next_tick: Option<u64>) -> Self {
    Self { handle_id, mode, deadline_tick, next_tick }
  }

  /// Returns the handle identifier.
  #[must_use]
  pub const fn handle_id(&self) -> u64 {
    self.handle_id
  }

  /// Returns the scheduling mode.
  #[must_use]
  pub const fn mode(&self) -> SchedulerMode {
    self.mode
  }

  /// Returns the recorded deadline tick.
  #[must_use]
  pub const fn deadline_tick(&self) -> u64 {
    self.deadline_tick
  }

  /// Returns the next periodic tick, if available.
  #[must_use]
  pub const fn next_tick(&self) -> Option<u64> {
    self.next_tick
  }
}
