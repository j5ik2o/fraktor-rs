use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::{
  core::dispatcher::{DispatchError, DispatchExecutor as CoreDispatchExecutor},
  std::dispatcher::DispatchShared,
};

#[cfg(test)]
mod tests;

/// Scheduler abstraction for driving dispatcher execution in the standard runtime.
pub trait DispatchExecutor: Send + Sync + 'static {
  /// Delegates dispatcher execution to the scheduler.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchError`] when the scheduler fails to enqueue the dispatcher for execution.
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError>;
}

impl<T> DispatchExecutor for T
where
  T: CoreDispatchExecutor<StdToolbox> + 'static,
{
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    CoreDispatchExecutor::execute(self, dispatcher)
  }
}
