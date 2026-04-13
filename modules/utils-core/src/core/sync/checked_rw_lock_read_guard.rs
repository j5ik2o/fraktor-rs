//! Read guard for [`CheckedSpinSyncRwLock`](super::CheckedSpinSyncRwLock).

use core::{mem::ManuallyDrop, ops::Deref, sync::atomic::Ordering};

use spin::RwLockReadGuard;

use super::checked_spin_sync_rwlock::{CheckedSpinSyncRwLock, STATE_FREE};

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
    self.parent.state.store(STATE_FREE, Ordering::Release);
  }
}
