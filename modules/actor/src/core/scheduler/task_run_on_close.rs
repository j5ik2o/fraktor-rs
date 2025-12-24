use crate::core::scheduler::task_run_error::TaskRunError;

/// Trait implemented by shutdown tasks executed after scheduler stops accepting work.
pub trait TaskRunOnClose: Send + Sync + 'static {
  /// Executes the task.
  ///
  /// Callers must guarantee exclusive access to the task object (e.g., by owning it or by
  /// synchronizing externally) before invoking this method.
  ///
  /// # Errors
  ///
  /// Returns `TaskRunError` if the task execution fails.
  fn run(&mut self) -> Result<(), TaskRunError>;
}
