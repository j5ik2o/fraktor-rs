//! Low-level executor abstraction shared by all dispatchers.
//!
//! `Executor` is the dispatcher-side seam between submitting a task closure and
//! whatever runtime (tokio, std::thread, embedded) actually executes it. The
//! trait follows the project-wide CQS rule: `execute` and `shutdown` mutate the
//! executor backend so they take `&mut self`.
//!
//! Production dispatchers always wrap an executor in
//! [`ExecutorShared`](super::ExecutorShared) so that the `&mut self` contract
//! is observed under a `SpinSyncMutex`.

use alloc::boxed::Box;

use super::execute_error::ExecuteError;

/// Submits closures to a runtime that owns the underlying threads.
///
/// Implementations must be `Send + Sync` so that the wrapping
/// [`ExecutorShared`](super::ExecutorShared) can hand them around between
/// threads. The `&mut self` receivers express the **command** semantics in CQS
/// terms; whether the implementation actually mutates internal state is up to
/// the backend (a `tokio::runtime::Handle` does not, while the `PinnedExecutor`
/// does take its sender via `Option::take` on shutdown).
pub trait Executor: Send + Sync {
  /// Submits the task closure for asynchronous execution.
  ///
  /// # Errors
  ///
  /// Returns [`ExecuteError`] when the backend cannot accept the task. Callers
  /// must roll back the mailbox CAS state when this happens.
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError>;

  /// Shuts the executor down, releasing the underlying worker resources.
  fn shutdown(&mut self);
}
