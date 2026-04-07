//! RAII guard returned by [`DebugSpinSyncMutex::lock`].

use std::{
  ops::{Deref, DerefMut},
  sync::atomic::{AtomicU64, Ordering},
};

use super::debug_spin_sync_mutex::UNLOCKED;

/// RAII guard returned by [`super::DebugSpinSyncMutex::lock`].
///
/// On drop, the guard atomically clears the recorded owner first, then
/// the inner spin guard releases the underlying mutex.
pub struct DebugSpinSyncMutexGuard<'a, T> {
  inner: spin::MutexGuard<'a, T>,
  owner: &'a AtomicU64,
}

impl<'a, T> DebugSpinSyncMutexGuard<'a, T> {
  /// Constructs a guard. Internal API used by `DebugSpinSyncMutex::lock`.
  pub(super) const fn new(inner: spin::MutexGuard<'a, T>, owner: &'a AtomicU64) -> Self {
    Self { inner, owner }
  }
}

impl<T> Deref for DebugSpinSyncMutexGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &T {
    &self.inner
  }
}

impl<T> DerefMut for DebugSpinSyncMutexGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut T {
    &mut self.inner
  }
}

impl<T> Drop for DebugSpinSyncMutexGuard<'_, T> {
  fn drop(&mut self) {
    // Clear the recorded owner before releasing the inner spin mutex.
    // The custom drop runs first; the `inner: spin::MutexGuard` field
    // drops afterwards (in declaration order), releasing the lock at
    // that point. A thread observing `owner == UNLOCKED` may then
    // attempt `lock()`, which still has to wait for the spin mutex
    // release that happens immediately after.
    self.owner.store(UNLOCKED, Ordering::Release);
  }
}
