use fraktor_utils_core_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{dispatch_error::DispatchError, dispatch_shared::DispatchSharedGeneric};

/// Abstraction for schedulers to hook dispatcher execution.
pub trait DispatchExecutor<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Delegates dispatcher execution to the scheduler.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchError`] when the scheduler rejects the submitted dispatcher task.
  fn execute(&self, dispatcher: DispatchSharedGeneric<TB>) -> Result<(), DispatchError>;

  /// Returns `true` if this executor supports blocking mailbox operations.
  ///
  /// Executors that run on a single thread or use cooperative scheduling should return `false`,
  /// as blocking operations can cause deadlocks. Multi-threaded executors can return `true`.
  ///
  /// This is used to validate that
  /// [`MailboxOverflowStrategy::Block`](crate::core::mailbox::MailboxOverflowStrategy::Block)
  /// is only used with compatible executors.
  fn supports_blocking(&self) -> bool {
    true
  }
}
