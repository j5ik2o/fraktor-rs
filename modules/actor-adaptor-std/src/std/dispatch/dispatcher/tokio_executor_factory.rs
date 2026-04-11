//! Factory that produces [`TokioExecutor`] handles wrapped in [`ExecutorShared`].

use fraktor_actor_core_rs::core::kernel::{
  dispatch::dispatcher::{ExecutorFactory, ExecutorShared},
  system::lock_provider::ActorLockProvider,
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::runtime::Handle;

use super::tokio_executor::TokioExecutor;

/// [`ExecutorFactory`] that yields [`TokioExecutor`] backends.
pub struct TokioExecutorFactory {
  handle:        Handle,
  lock_provider: ArcShared<dyn ActorLockProvider>,
}

impl TokioExecutorFactory {
  /// Builds a factory bound to the supplied Tokio runtime handle.
  #[must_use]
  pub fn new(handle: Handle, lock_provider: &ArcShared<dyn ActorLockProvider>) -> Self {
    Self { handle, lock_provider: lock_provider.clone() }
  }
}

impl ExecutorFactory for TokioExecutorFactory {
  fn create(&self, _id: &str) -> ExecutorShared {
    self.lock_provider.create_executor_shared(Box::new(TokioExecutor::new(self.handle.clone())))
  }
}
