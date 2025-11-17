use crate::core::scheduler::task_run_error::TaskRunError;

/// Trait implemented by shutdown tasks executed after scheduler stops accepting work.
pub trait TaskRunOnClose: Send + Sync + 'static {
  /// Executes the task.
  ///
  /// # Errors
  ///
  /// Returns `TaskRunError` if the task execution fails.
  fn run(&self) -> Result<(), TaskRunError>;
}
