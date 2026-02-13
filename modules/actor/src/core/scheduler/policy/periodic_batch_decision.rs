//! Decisions returned when preparing periodic batches.

use crate::core::scheduler::{ExecutionBatch, SchedulerWarning};

/// Outcome of evaluating a periodic scheduler job.
pub(crate) enum PeriodicBatchDecision {
  /// Job continues execution with the provided batch metadata.
  Execute {
    /// Batch metadata describing the run(s).
    batch:   ExecutionBatch,
    /// Optional warning emitted alongside execution.
    warning: Option<SchedulerWarning>,
  },
  /// Job is cancelled due to backlog or policy violations.
  Cancel {
    /// Warning describing the cancellation reason.
    warning: SchedulerWarning,
  },
}
