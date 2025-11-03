use super::dispatch_handle::DispatchHandle;
use crate::RuntimeToolbox;

/// Abstraction for schedulers to hook dispatcher execution.
pub trait DispatchExecutor<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Delegates dispatcher execution to the scheduler.
  fn execute(&self, dispatcher: DispatchHandle<TB>);
}
