//! Read guard for [`CheckedSpinSyncRwLock`](super::CheckedSpinSyncRwLock).

use core::{mem::ManuallyDrop, ops::Deref};

use spin::RwLockReadGuard;

use super::checked_spin_sync_rwlock::CheckedSpinSyncRwLock;

/// Read guard for [`CheckedSpinSyncRwLock`].
pub struct CheckedRwLockReadGuard<'a, T> {
  pub(super) parent: &'a CheckedSpinSyncRwLock<T>,
  pub(super) guard:  ManuallyDrop<RwLockReadGuard<'a, T>>,
}

impl<T> Deref for CheckedRwLockReadGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> Drop for CheckedRwLockReadGuard<'_, T> {
  fn drop(&mut self) {
    unsafe { ManuallyDrop::drop(&mut self.guard) };
    *self.parent.owner.lock().unwrap_or_else(|e| e.into_inner()) = None;
  }
}
