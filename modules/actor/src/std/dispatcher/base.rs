use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  core::dispatcher::{DispatchError, DispatchExecutorRunner},
  std::dispatcher::DispatchShared,
};

#[cfg(test)]
mod tests;

/// Scheduler abstraction for driving dispatcher execution in the standard runtime.
///
/// Unlike `core::DispatchExecutor` which uses `&mut self`, this trait uses `&self`
/// because standard runtime executors can leverage OS-level concurrency primitives
/// (threads, async runtimes) that handle synchronization internally.
pub trait DispatchExecutor: Send + Sync + 'static {
  /// Delegates dispatcher execution to the scheduler.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchError`] when the scheduler fails to enqueue the dispatcher for execution.
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError>;
}

impl DispatchExecutor for DispatchExecutorRunner<StdToolbox> {
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.submit(dispatcher)
  }
}
