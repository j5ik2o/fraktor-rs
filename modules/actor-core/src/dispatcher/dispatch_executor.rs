use super::{dispatch_error::DispatchError, dispatch_shared::DispatchSharedGeneric};
use crate::RuntimeToolbox;

/// Abstraction for schedulers to hook dispatcher execution.
pub trait DispatchExecutor<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Delegates dispatcher execution to the scheduler.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchError`] when the scheduler rejects the submitted dispatcher task.
  fn execute(&self, dispatcher: DispatchSharedGeneric<TB>) -> Result<(), DispatchError>;
}
