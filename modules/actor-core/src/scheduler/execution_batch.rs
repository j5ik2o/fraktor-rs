//! Metadata describing how many runs were triggered for a scheduled task.

use core::num::NonZeroU32;

/// Execution modes used to interpret batch data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchMode {
  /// Single-shot execution.
  OneShot,
  /// Fixed-rate periodic execution.
  FixedRate,
  /// Fixed-delay periodic execution.
  FixedDelay,
}

/// Execution metadata shared with scheduler tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionBatch {
  runs:        NonZeroU32,
  missed_runs: u32,
  mode:        BatchMode,
}

impl ExecutionBatch {
  /// Creates a new batch description.
  #[must_use]
  pub const fn new(runs: NonZeroU32, missed_runs: u32, mode: BatchMode) -> Self {
    Self { runs, missed_runs, mode }
  }

  /// Batch describing a single run with no accumulated backlog.
  #[must_use]
  pub fn oneshot() -> Self {
    let runs = NonZeroU32::new(1).expect("non-zero");
    Self::new(runs, 0, BatchMode::OneShot)
  }

  /// Batch describing periodic execution.
  #[must_use]
  pub fn periodic(runs: NonZeroU32, missed_runs: u32, mode: BatchMode) -> Self {
    Self::new(runs, missed_runs, mode)
  }

  /// Number of runs represented by this batch.
  #[must_use]
  pub const fn runs(&self) -> NonZeroU32 {
    self.runs
  }

  /// Number of missed runs folded into this batch.
  #[must_use]
  pub const fn missed_runs(&self) -> u32 {
    self.missed_runs
  }

  /// Returns the batch mode.
  #[must_use]
  pub const fn mode(&self) -> BatchMode {
    self.mode
  }
}
