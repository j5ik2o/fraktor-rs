//! Runnable trait executed by scheduler-delivered closures.

use super::execution_batch::ExecutionBatch;

/// Trait implemented by runnable tasks scheduled through the scheduler APIs.
pub trait SchedulerRunnable: Send + Sync + 'static {
  /// Executes the runnable with the associated batch metadata.
  fn run(&self, batch: &ExecutionBatch);
}

impl<F> SchedulerRunnable for F
where
  F: Fn(&ExecutionBatch) + Send + Sync + 'static,
{
  fn run(&self, batch: &ExecutionBatch) {
    (self)(batch);
  }
}
