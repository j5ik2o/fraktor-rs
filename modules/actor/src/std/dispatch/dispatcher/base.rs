use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  core::dispatch::dispatcher::{DispatchError, DispatchExecutorRunnerGeneric},
  std::dispatch::dispatcher::DispatchShared,
};

#[cfg(test)]
mod tests;

/// Scheduler abstraction for driving dispatcher execution in the standard runtime.
///
/// Requires `&mut self` and does not hold internal locks; callers must provide
/// external synchronization (e.g., via `StdSyncMutex`).
pub trait DispatchExecutor: Send + 'static {
  /// Delegates dispatcher execution to the scheduler.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchError`] when the scheduler fails to enqueue the dispatcher for execution.
  fn execute(&mut self, dispatcher: DispatchShared) -> Result<(), DispatchError>;
}

impl DispatchExecutor for DispatchExecutorRunnerGeneric<StdToolbox> {
  fn execute(&mut self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.submit(dispatcher)
  }
}
