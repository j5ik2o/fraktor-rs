use core::{
  mem::ManuallyDrop,
  ops::{Deref, DerefMut},
  sync::atomic::Ordering,
};

use super::debug_spin_sync_mutex::DebugSpinSyncMutex;

/// Guard for [`DebugSpinSyncMutex`](super::DebugSpinSyncMutex).
pub struct DebugSpinSyncMutexGuard<'a, T> {
  pub(super) parent: &'a DebugSpinSyncMutex<T>,
  pub(super) guard:  ManuallyDrop<spin::MutexGuard<'a, T>>,
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
    // 実際の mutex を先に解放してから owner をクリアする。
    // この順序により、owner=0 を観測した他スレッドは必ず実ロックも
    // 解放済みとなり、debug 検知の TOCTOU 窓を作らない。
    // SAFETY: Drop は 1 回だけ呼ばれ、guard はまだ有効。
    unsafe { ManuallyDrop::drop(&mut self.guard) };
    self.parent.owner.store(0, Ordering::Release);
  }
}
