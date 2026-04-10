use core::ops::{Deref, DerefMut};

use super::debug_spin_sync_mutex::DebugSpinSyncMutex;

/// Guard for [`DebugSpinSyncMutex`](super::DebugSpinSyncMutex).
pub struct DebugSpinSyncMutexGuard<'a, T> {
  pub(super) parent: &'a DebugSpinSyncMutex<T>,
  pub(super) guard:  spin::MutexGuard<'a, T>,
}

impl<T> Deref for DebugSpinSyncMutexGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> DerefMut for DebugSpinSyncMutexGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

impl<T> Drop for DebugSpinSyncMutexGuard<'_, T> {
  fn drop(&mut self) {
    self.parent.owner.store(0, core::sync::atomic::Ordering::Release);
  }
}
