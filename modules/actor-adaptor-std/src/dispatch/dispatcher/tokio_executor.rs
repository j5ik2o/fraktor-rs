//! [`Executor`] backed by a Tokio runtime handle.

#[cfg(test)]
#[path = "tokio_executor_test.rs"]
mod tests;

use alloc::boxed::Box;

use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{ExecuteError, Executor};
use tokio::runtime::Handle;

/// Submits tasks to a Tokio runtime via `spawn_blocking`.
///
/// `&mut self` is required by the [`Executor`] trait CQS contract; the
/// underlying [`Handle`] is `Clone`-friendly so this implementation does not
/// actually mutate any state. The `shutdown` hook is a no-op since runtime
/// teardown is owned by the runtime itself.
pub struct TokioExecutor {
  handle: Handle,
}

impl TokioExecutor {
  /// Creates a new executor bound to the supplied Tokio runtime handle.
  #[must_use]
  pub const fn new(handle: Handle) -> Self {
    Self { handle }
  }
}

impl Executor for TokioExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    // `spawn_blocking` returns a `JoinHandle` which we deliberately drop:
    // task lifetime is owned by Tokio's blocking pool, and the dispatcher
    // does not need to await completion to keep its scheduling guarantees.
    drop(self.handle.spawn_blocking(task));
    Ok(())
  }

  fn shutdown(&mut self) {
    // The Tokio runtime owns its lifecycle. There is no per-handle shutdown
    // path; if the dispatcher wants to stop accepting tasks it should drop
    // the runtime separately.
  }
}
