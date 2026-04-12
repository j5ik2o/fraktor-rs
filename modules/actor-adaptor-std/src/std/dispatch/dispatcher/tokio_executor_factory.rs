//! Factory that produces [`TokioExecutor`] handles wrapped in [`ExecutorShared`].

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::{
  ExecutorFactory, ExecutorShared, ExecutorSharedFactory, TrampolineState,
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::runtime::Handle;

use super::tokio_executor::TokioExecutor;

/// [`ExecutorFactory`] that yields [`TokioExecutor`] backends.
pub struct TokioExecutorFactory {
  handle:                  Handle,
  executor_shared_factory: ArcShared<dyn ExecutorSharedFactory>,
}

impl TokioExecutorFactory {
  /// Builds a factory bound to the supplied Tokio runtime handle.
  #[must_use]
  pub fn new(handle: Handle, executor_shared_factory: &ArcShared<dyn ExecutorSharedFactory>) -> Self {
    Self { handle, executor_shared_factory: executor_shared_factory.clone() }
  }
}

impl ExecutorFactory for TokioExecutorFactory {
  fn create(&self, _id: &str) -> ExecutorShared {
    self
      .executor_shared_factory
      .create_executor_shared(Box::new(TokioExecutor::new(self.handle.clone())), TrampolineState::new())
  }
}
