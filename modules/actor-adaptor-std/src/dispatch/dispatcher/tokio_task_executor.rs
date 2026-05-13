//! [`Executor`] backed by Tokio task scheduling.

#[cfg(test)]
#[path = "tokio_task_executor_test.rs"]
mod tests;

use alloc::boxed::Box;

use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{ExecuteError, Executor};
use tokio::runtime::Handle;

/// Submits actor dispatcher tasks to a Tokio runtime via `spawn`.
///
/// This executor is intended for the default actor dispatcher when the actor
/// work itself must stay on Tokio worker tasks. Use [`TokioExecutor`](super::TokioExecutor)
/// for work that should be delegated to Tokio's blocking pool.
pub struct TokioTaskExecutor {
  handle: Handle,
}

impl TokioTaskExecutor {
  /// Creates a new task executor bound to the supplied Tokio runtime handle.
  #[must_use]
  pub const fn new(handle: Handle) -> Self {
    Self { handle }
  }
}

impl Executor for TokioTaskExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    drop(self.handle.spawn(async move {
      task();
    }));
    Ok(())
  }

  fn shutdown(&mut self) {}
}
