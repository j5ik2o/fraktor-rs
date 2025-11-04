mod dispatcher_config;
mod types;
use cellactor_actor_core_rs::dispatcher::DispatchExecutor as CoreDispatchExecutor;
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::StdToolbox;
pub use dispatcher_config::DispatcherConfig;
pub use types::*;

/// Scheduler abstraction for driving dispatcher execution in the standard runtime.
pub trait DispatchExecutor: Send + Sync + 'static {
  /// Delegates dispatcher execution to the scheduler.
  fn execute(&self, dispatcher: DispatchHandle);
}

impl<T> DispatchExecutor for T
where
  T: CoreDispatchExecutor<StdToolbox> + 'static,
{
  fn execute(&self, dispatcher: DispatchHandle) {
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

impl CoreDispatchExecutor<StdToolbox> for DispatchExecutorAdapter {
  fn execute(&self, dispatcher: DispatchHandle) {
    self.inner.execute(dispatcher);
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

impl DispatchExecutor for CoreDispatchExecutorAdapter {
  fn execute(&self, dispatcher: DispatchHandle) {
    self.inner.execute(dispatcher);
  }
}
