//! Thin shared wrapper exposing [`Executor`] to multiple owners.
//!
//! `ExecutorShared` is the only sanctioned way to share a `Box<dyn Executor>`
//! between dispatchers and the rest of the runtime. Internal mutability is
//! confined to the `RuntimeMutex` housed inside `ArcShared`, matching the
//! AShared pattern documented in `docs/guides/shared_vs_handle.md`.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::{execute_error::ExecuteError, executor::Executor};

/// Multi-owner handle for a boxed [`Executor`].
///
/// Cloning the wrapper is cheap (`ArcShared::clone`) and does not duplicate the
/// underlying executor. The lock is held only for the duration of `execute` /
/// `shutdown` / `supports_blocking` calls and never escapes a closure.
pub struct ExecutorShared {
  inner: ArcShared<RuntimeMutex<Box<dyn Executor>>>,
}

impl ExecutorShared {
  /// Wraps the provided executor in a shareable handle.
  #[must_use]
  pub fn new<E: Executor + 'static>(executor: E) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(Box::new(executor) as Box<dyn Executor>)) }
  }

  /// Wraps an already-boxed executor in a shareable handle.
  #[must_use]
  pub fn from_boxed(executor: Box<dyn Executor>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(executor)) }
  }

  /// Submits the task to the inner executor.
  ///
  /// # Errors
  ///
  /// Returns [`ExecuteError`] when the underlying executor rejects the task.
  pub fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    self.with_write(|inner| inner.execute(task))
  }

  /// Shuts the inner executor down.
  pub fn shutdown(&self) {
    self.with_write(|inner| inner.shutdown());
  }

  /// Returns whether the inner executor accepts blocking workloads.
  #[must_use]
  pub fn supports_blocking(&self) -> bool {
    self.with_read(|inner| inner.supports_blocking())
  }
}

impl Clone for ExecutorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn Executor>> for ExecutorShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Executor>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Executor>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
