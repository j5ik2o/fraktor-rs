//! Thin shared wrapper for `Scheduler`.
//!
//! Hides the `ArcShared<ToolboxMutex<...>>` internals and exposes only
//! the `with_read` / `with_write` closure API.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::Scheduler;

/// Thin shared wrapper around `ArcShared<ToolboxMutex<Scheduler<..>>>`.
pub struct SchedulerSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxRwLock<Scheduler<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for SchedulerSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SchedulerSharedGeneric<TB> {
  /// Wrap an existing shared mutex.
  #[must_use]
  pub const fn new(inner: ArcShared<ToolboxRwLock<Scheduler<TB>, TB>>) -> Self {
    Self { inner }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Scheduler<TB>> for SchedulerSharedGeneric<TB> {
  #[inline]
  fn with_read<R>(&self, f: impl FnOnce(&Scheduler<TB>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  #[inline]
  fn with_write<R>(&self, f: impl FnOnce(&mut Scheduler<TB>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}

/// Alias specialized with the default `NoStdToolbox`.
pub type SchedulerShared = SchedulerSharedGeneric<NoStdToolbox>;
