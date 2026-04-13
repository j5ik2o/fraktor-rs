//! Guard for [`CheckedSpinSyncMutex`](super::CheckedSpinSyncMutex).

use core::{
  mem::ManuallyDrop,
  ops::{Deref, DerefMut},
  sync::atomic::Ordering,
};

use spin::MutexGuard;

use super::checked_spin_sync_mutex::CheckedSpinSyncMutex;

/// Guard for [`CheckedSpinSyncMutex`].
pub struct CheckedSpinSyncMutexGuard<'a, T> {
  pub(super) parent: &'a CheckedSpinSyncMutex<T>,
  pub(super) guard:  ManuallyDrop<MutexGuard<'a, T>>,
}

impl<T> Deref for CheckedSpinSyncMutexGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> DerefMut for CheckedSpinSyncMutexGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

impl<T> Drop for CheckedSpinSyncMutexGuard<'_, T> {
  fn drop(&mut self) {
    // Release the real lock first, then clear the flag.
    // SAFETY: Drop is called exactly once and the guard is still valid.
    unsafe { ManuallyDrop::drop(&mut self.guard) };
    self.parent.locked.store(false, Ordering::Release);
  }
}
