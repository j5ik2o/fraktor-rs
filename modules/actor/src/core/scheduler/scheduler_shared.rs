//! Thin shared wrapper type for `Scheduler` guarded by a toolbox mutex.
//!
//! Provides only minimal helpers: lock the mutex, or get the raw shared handle.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::Scheduler;

/// Thin shared wrapper around `ArcShared<ToolboxMutex<Scheduler<..>>>`.
pub struct SchedulerSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for SchedulerSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SchedulerSharedGeneric<TB> {
  /// Wrap an existing shared mutex.
  #[must_use]
  pub const fn new(inner: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>) -> Self {
    Self { inner }
  }

  /// Run a closure while holding the scheduler mutex.
  #[inline]
  pub fn with_mut<R>(&self, f: impl FnOnce(&mut Scheduler<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }

  #[allow(dead_code)]
  pub(crate) fn lock_guard(&self) -> <ToolboxMutex<Scheduler<TB>, TB> as SyncMutexLike<Scheduler<TB>>>::Guard<'_> {
    self.inner.lock()
  }
}

/// Alias specialized with the default `NoStdToolbox`.
pub type SchedulerShared = SchedulerSharedGeneric<NoStdToolbox>;
