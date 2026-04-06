/// Summary returned by scheduler shutdown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskRunSummary {
  /// Number of tasks that completed successfully.
  pub executed_tasks: usize,
  /// Number of tasks that failed.
  pub failed_tasks:   usize,
}
