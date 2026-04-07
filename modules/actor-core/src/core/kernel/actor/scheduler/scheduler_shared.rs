//! Thin shared wrapper for `Scheduler`.
//!
//! Hides the `ArcShared<RuntimeRwLock<...>>` internals and exposes only
//! the `with_read` / `with_write` closure API.

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeRwLock, SharedAccess};

use super::Scheduler;

/// Thin shared wrapper around `ArcShared<RuntimeRwLock<Scheduler<..>>>`.
pub struct SchedulerShared {
  inner: ArcShared<RuntimeRwLock<Scheduler>>,
}

impl Clone for SchedulerShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SchedulerShared {
  /// Wrap an existing shared mutex.
  #[must_use]
  pub const fn new(inner: ArcShared<RuntimeRwLock<Scheduler>>) -> Self {
    Self { inner }
  }
}

impl SharedAccess<Scheduler> for SchedulerShared {
  #[inline]
  fn with_read<R>(&self, f: impl FnOnce(&Scheduler) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  #[inline]
  fn with_write<R>(&self, f: impl FnOnce(&mut Scheduler) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
