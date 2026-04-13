//! Guard for [`CheckedSpinSyncMutex`](super::CheckedSpinSyncMutex).

use core::{
  mem::ManuallyDrop,
  ops::{Deref, DerefMut},
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
    // Clear the owner while still holding the inner lock so that another
    // thread acquiring inner right after cannot have its owner overwritten.
    *self.parent.owner.lock().unwrap_or_else(|e| e.into_inner()) = None;
    // SAFETY: Drop is called exactly once and the guard is still valid.
    unsafe { ManuallyDrop::drop(&mut self.guard) };
  }
}
