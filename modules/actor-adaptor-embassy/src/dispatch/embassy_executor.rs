//! [`Executor`] backed by an Embassy-ready bounded queue.

#[cfg(test)]
#[path = "embassy_executor_test.rs"]
mod tests;

use alloc::boxed::Box;

use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{ExecuteError, Executor};

use super::embassy_executor_shared::EmbassyExecutorShared;

/// Executor that enqueues actor mailbox work for an Embassy worker task.
pub struct EmbassyExecutor<const N: usize> {
  shared: EmbassyExecutorShared<N>,
}

impl<const N: usize> EmbassyExecutor<N> {
  pub(crate) const fn new(shared: EmbassyExecutorShared<N>) -> Self {
    Self { shared }
  }
}

impl<const N: usize> Executor for EmbassyExecutor<N> {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    self.shared.enqueue(task)
  }

  fn shutdown(&mut self) {
    self.shared.shutdown();
  }
}
