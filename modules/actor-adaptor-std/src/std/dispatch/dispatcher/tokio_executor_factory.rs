//! Factory that produces [`TokioExecutor`] handles wrapped in [`ExecutorShared`].

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::{
  ExecutorFactory, ExecutorShared, TrampolineState,
};
use tokio::runtime::Handle;

use super::tokio_executor::TokioExecutor;

/// [`ExecutorFactory`] that yields [`TokioExecutor`] backends.
pub struct TokioExecutorFactory {
  handle: Handle,
}

impl TokioExecutorFactory {
  /// Builds a factory bound to the supplied Tokio runtime handle.
  #[must_use]
  pub fn new(handle: Handle) -> Self {
    Self { handle }
  }
}

impl ExecutorFactory for TokioExecutorFactory {
  fn create(&self, _id: &str) -> ExecutorShared {
    ExecutorShared::new(Box::new(TokioExecutor::new(self.handle.clone())), TrampolineState::new())
  }
}
