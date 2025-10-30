use super::dispatch_handle::DispatchHandle;

/// Abstraction for schedulers to hook dispatcher execution.
pub trait DispatchExecutor: Send + Sync {
  /// Delegates dispatcher execution to the scheduler.
  fn execute(&self, dispatcher: DispatchHandle);
}
