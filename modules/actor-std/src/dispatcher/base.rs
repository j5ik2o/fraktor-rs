use fraktor_actor_core_rs::core::dispatcher::{DispatchError, DispatchExecutor as CoreDispatchExecutor};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use crate::dispatcher::DispatchShared;

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

pub(crate) struct DispatchExecutorAdapter {
  inner: ArcShared<dyn DispatchExecutor>,
}

impl DispatchExecutorAdapter {
  pub(crate) fn new(inner: ArcShared<dyn DispatchExecutor>) -> Self {
    Self { inner }
  }
}

pub(crate) struct CoreDispatchExecutorAdapter {
  inner: ArcShared<dyn CoreDispatchExecutor<StdToolbox>>,
}

impl CoreDispatchExecutorAdapter {
  pub(crate) fn new(inner: ArcShared<dyn CoreDispatchExecutor<StdToolbox>>) -> Self {
    Self { inner }
  }
}

impl CoreDispatchExecutor<StdToolbox> for DispatchExecutorAdapter {
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.inner.execute(dispatcher)
  }
}

impl DispatchExecutor for CoreDispatchExecutorAdapter {
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.inner.execute(dispatcher)
  }
}
