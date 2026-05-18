//! Factory that produces [`TokioTaskExecutor`] handles wrapped in [`ExecutorShared`].

use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{ExecutorFactory, ExecutorShared, TrampolineState};
use tokio::runtime::Handle;

use super::tokio_task_executor::TokioTaskExecutor;

/// [`ExecutorFactory`] that yields [`TokioTaskExecutor`] backends.
pub struct TokioTaskExecutorFactory {
  handle: Handle,
}

impl TokioTaskExecutorFactory {
  /// Builds a factory bound to the supplied Tokio runtime handle.
  #[must_use]
  pub fn new(handle: Handle) -> Self {
    Self { handle }
  }
}

impl ExecutorFactory for TokioTaskExecutorFactory {
  fn create(&self, _id: &str) -> ExecutorShared {
    ExecutorShared::new(Box::new(TokioTaskExecutor::new(self.handle.clone())), TrampolineState::new())
  }
}
