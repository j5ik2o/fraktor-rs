//! Re-entry detecting spin-based mutex for debug/test instrumentation (no_std compatible).

#[cfg(test)]
mod tests;

use core::{
  mem::ManuallyDrop,
  sync::atomic::{AtomicBool, Ordering},
};

use super::{LockDriver, checked_spin_sync_mutex_guard::CheckedSpinSyncMutexGuard, spin_sync_mutex::SpinSyncMutex};

/// Spin-based mutex with re-entry detection.
///
/// Wraps [`SpinSyncMutex`] and adds an `AtomicBool` flag that panics if `lock()`
/// is called while the mutex is already held. This catches recursive / re-entrant
/// locking that would otherwise deadlock silently on `spin::Mutex`.
///
/// Unlike [`DebugSpinSyncMutex`](fraktor_utils_adaptor_std_rs) this variant
/// does **not** require `std::thread` and works in `no_std` environments.
/// The trade-off is that the panic message cannot include the owning thread ID.
pub struct CheckedSpinSyncMutex<T> {
  pub(super) inner:  SpinSyncMutex<T>,
  pub(super) locked: AtomicBool,
}

unsafe impl<T: Send> Send for CheckedSpinSyncMutex<T> {}
unsafe impl<T: Send> Sync for CheckedSpinSyncMutex<T> {}

impl<T> CheckedSpinSyncMutex<T> {
  /// Creates a new checked mutex.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self { inner: SpinSyncMutex::new(value), locked: AtomicBool::new(false) }
  }

  /// Acquires the mutex, panicking on re-entry.
  ///
  /// # Panics
  ///
  /// Panics if the mutex is already held (re-entrant lock detected).
  pub fn lock(&self) -> CheckedSpinSyncMutexGuard<'_, T> {
    assert!(!self.locked.swap(true, Ordering::Acquire), "CheckedSpinSyncMutex: re-entrant lock detected");
    let guard = self.inner.lock();
    CheckedSpinSyncMutexGuard { parent: self, guard: ManuallyDrop::new(guard) }
  }

  /// Consumes the mutex and returns the inner value.
  pub fn into_inner(self) -> T {
    self.inner.into_inner()
  }
}

impl<T> LockDriver<T> for CheckedSpinSyncMutex<T> {
  type Guard<'a>
    = CheckedSpinSyncMutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self {
    Self::new(value)
  }

  fn lock(&self) -> Self::Guard<'_> {
    CheckedSpinSyncMutex::lock(self)
  }

  fn into_inner(self) -> T {
    CheckedSpinSyncMutex::into_inner(self)
  }
}
