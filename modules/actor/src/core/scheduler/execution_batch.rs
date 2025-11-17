//! Metadata describing how many runs were triggered for a scheduled task.

use core::num::NonZeroU32;

use crate::core::scheduler::BatchMode;

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
  pub const fn oneshot() -> Self {
    // SAFETY: 1 is non-zero
    let runs = unsafe { NonZeroU32::new_unchecked(1) };
    Self::new(runs, 0, BatchMode::OneShot)
  }

  /// Batch describing periodic execution.
  #[must_use]
  pub const fn periodic(runs: NonZeroU32, missed_runs: u32, mode: BatchMode) -> Self {
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
