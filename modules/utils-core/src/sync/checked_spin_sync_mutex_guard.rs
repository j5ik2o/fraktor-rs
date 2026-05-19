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
    // inner ロックを保持中に所有者をクリアする。
    // 解放直後に別スレッドが inner を取得しても owner が上書きされない。
    *self.parent.owner.lock().unwrap_or_else(|e| e.into_inner()) = None;
    // SAFETY: drop は一度だけ呼ばれ、guard はまだ有効。
    unsafe { ManuallyDrop::drop(&mut self.guard) };
  }
}
