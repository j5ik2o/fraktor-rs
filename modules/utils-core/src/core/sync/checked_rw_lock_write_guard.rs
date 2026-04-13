//! Write guard for [`CheckedSpinSyncRwLock`](super::CheckedSpinSyncRwLock).
#![allow(cfg_std_forbid)]

use core::{
  mem::ManuallyDrop,
  ops::{Deref, DerefMut},
};

use spin::RwLockWriteGuard;

use super::checked_spin_sync_rwlock::CheckedSpinSyncRwLock;

/// Write guard for [`CheckedSpinSyncRwLock`].
pub struct CheckedRwLockWriteGuard<'a, T> {
  pub(super) parent: &'a CheckedSpinSyncRwLock<T>,
  pub(super) guard:  ManuallyDrop<RwLockWriteGuard<'a, T>>,
}

impl<T> Deref for CheckedRwLockWriteGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> DerefMut for CheckedRwLockWriteGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

impl<T> Drop for CheckedRwLockWriteGuard<'_, T> {
  fn drop(&mut self) {
    // inner ロックを保持中に所有者をクリアする。
    // 解放直後に別スレッドが inner を取得しても owner が上書きされない。
    let mut state = self.parent.owner.lock().unwrap_or_else(|e| e.into_inner());
    state.write_owner = None;
    drop(state);
    // SAFETY: drop は一度だけ呼ばれ、guard はまだ有効。
    unsafe { ManuallyDrop::drop(&mut self.guard) };
  }
}
