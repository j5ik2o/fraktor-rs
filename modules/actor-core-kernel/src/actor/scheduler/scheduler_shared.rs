//! Thin shared wrapper for `Scheduler`.
//!
//! Hides the `SharedRwLock<...>` internals and exposes only
//! the `with_read` / `with_write` closure API.

use fraktor_utils_core_rs::sync::{SharedAccess, SharedRwLock};

use super::Scheduler;

/// Thin shared wrapper around [`SharedRwLock<Scheduler>`].
///
/// External callers obtain this handle from
/// [`crate::actor::scheduler::SchedulerContext`] instead of
/// constructing it from the raw lock.
///
/// ```compile_fail
/// use fraktor_actor_core_kernel_rs::actor::scheduler::{Scheduler, SchedulerConfig, SchedulerShared};
/// use fraktor_utils_core_rs::sync::{DefaultRwLock, SharedRwLock};
///
/// let scheduler = Scheduler::new(SchedulerConfig::default());
/// let _ = SchedulerShared::new(SharedRwLock::new_with_driver::<DefaultRwLock<_>>(scheduler));
/// ```
pub struct SchedulerShared {
  inner: SharedRwLock<Scheduler>,
}

impl Clone for SchedulerShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SchedulerShared {
  /// Wrap an existing shared rw lock.
  #[must_use]
  pub(crate) const fn new(inner: SharedRwLock<Scheduler>) -> Self {
    Self { inner }
  }
}

impl SharedAccess<Scheduler> for SchedulerShared {
  #[inline]
  fn with_read<R>(&self, f: impl FnOnce(&Scheduler) -> R) -> R {
    self.inner.with_read(f)
  }

  #[inline]
  fn with_write<R>(&self, f: impl FnOnce(&mut Scheduler) -> R) -> R {
    self.inner.with_write(f)
  }
}
