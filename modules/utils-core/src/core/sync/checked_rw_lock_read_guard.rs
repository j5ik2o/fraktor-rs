//! Read guard for [`CheckedSpinSyncRwLock`](super::CheckedSpinSyncRwLock).
#![allow(cfg_std_forbid)]

use core::{mem::ManuallyDrop, ops::Deref};

use spin::RwLockReadGuard;
use std::thread;

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
    // Decrement the reader count while still holding the inner lock.
    let current = thread::current().id();
    let mut state = self.parent.owner.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(count) = state.reader_counts.get_mut(&current) {
      *count -= 1;
      if *count == 0 {
        state.reader_counts.remove(&current);
      }
    }
    drop(state);
    // SAFETY: Drop is called exactly once and the guard is still valid.
    unsafe { ManuallyDrop::drop(&mut self.guard) };
  }
}
