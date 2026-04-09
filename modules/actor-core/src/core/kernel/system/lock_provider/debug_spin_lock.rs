//! Panic-on-contention spin lock used by the debug actor lock provider.

use core::{
  fmt,
  ops::{Deref, DerefMut},
  sync::atomic::{AtomicBool, Ordering},
};

use fraktor_utils_core_rs::core::sync::SpinSyncMutex;

pub(crate) struct DebugSpinLock<T> {
  locked: AtomicBool,
  inner:  SpinSyncMutex<T>,
  label:  &'static str,
}

impl<T> DebugSpinLock<T> {
  pub(crate) const fn new(value: T, label: &'static str) -> Self {
    Self { locked: AtomicBool::new(false), inner: SpinSyncMutex::new(value), label }
  }

  pub(crate) fn lock(&self) -> DebugSpinLockGuard<'_, T> {
    assert!(
      !self.locked.swap(true, Ordering::AcqRel),
      "debug actor lock provider detected re-entrant or contended lock acquisition: {}",
      self.label
    );
    let guard = self.inner.lock();
    DebugSpinLockGuard { locked: &self.locked, guard }
  }
}

pub(crate) struct DebugSpinLockGuard<'a, T> {
  locked: &'a AtomicBool,
  guard:  spin::MutexGuard<'a, T>,
}

impl<T> Deref for DebugSpinLockGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> DerefMut for DebugSpinLockGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

impl<T> Drop for DebugSpinLockGuard<'_, T> {
  fn drop(&mut self) {
    self.locked.store(false, Ordering::Release);
  }
}

impl<T: fmt::Debug> fmt::Debug for DebugSpinLock<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("DebugSpinLock").field("label", &self.label).finish_non_exhaustive()
  }
}
