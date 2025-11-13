//! Metadata describing how many runs were triggered for a scheduled task.

use core::num::NonZeroU32;

/// Execution metadata shared with scheduler tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionBatch {
  runs:        NonZeroU32,
  missed_runs: u32,
}

impl ExecutionBatch {
  /// Creates a new batch description.
  #[must_use]
  pub const fn new(runs: NonZeroU32, missed_runs: u32) -> Self {
    Self { runs, missed_runs }
  }

  /// Batch describing a single run with no accumulated backlog.
  #[must_use]
  pub fn once() -> Self {
    let runs = NonZeroU32::new(1).expect("non-zero");
    Self::new(runs, 0)
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
}
