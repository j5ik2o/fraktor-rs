//! Factory for Embassy dispatcher executors.

use alloc::boxed::Box;

use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{ExecutorFactory, ExecutorShared, TrampolineState};

use super::{
  embassy_executor::EmbassyExecutor, embassy_executor_driver::EmbassyExecutorDriver,
  embassy_executor_shared::EmbassyExecutorShared,
};

/// [`ExecutorFactory`] that creates [`EmbassyExecutor`] instances sharing one Embassy queue.
pub struct EmbassyExecutorFactory<const N: usize> {
  shared: EmbassyExecutorShared<N>,
}

impl<const N: usize> EmbassyExecutorFactory<N> {
  /// Creates a new factory with an empty bounded ready queue.
  #[must_use]
  pub fn new() -> Self {
    Self { shared: EmbassyExecutorShared::new() }
  }

  /// Creates a driver handle for the factory's ready queue.
  #[must_use]
  pub fn driver(&self) -> EmbassyExecutorDriver<N> {
    EmbassyExecutorDriver::new(self.shared.clone())
  }
}

impl<const N: usize> Default for EmbassyExecutorFactory<N> {
  fn default() -> Self {
    Self::new()
  }
}

impl<const N: usize> Clone for EmbassyExecutorFactory<N> {
  fn clone(&self) -> Self {
    Self { shared: self.shared.clone() }
  }
}

impl<const N: usize> ExecutorFactory for EmbassyExecutorFactory<N> {
  fn create(&self, _id: &str) -> ExecutorShared {
    ExecutorShared::new(Box::new(EmbassyExecutor::new(self.shared.clone())), TrampolineState::new())
  }
}
