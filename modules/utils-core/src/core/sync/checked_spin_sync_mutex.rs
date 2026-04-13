//! Re-entry detecting spin-based mutex for debug/test instrumentation (no_std compatible).

#[cfg(test)]
mod tests;

use core::{
  mem::ManuallyDrop,
  ops::{Deref, DerefMut},
  sync::atomic::{AtomicBool, Ordering},
};

use spin::MutexGuard;

use super::{LockDriver, spin_sync_mutex::SpinSyncMutex};

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
  inner:  SpinSyncMutex<T>,
  locked: AtomicBool,
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
    assert!(
      !self.locked.swap(true, Ordering::Acquire),
      "CheckedSpinSyncMutex: re-entrant lock detected"
    );
    let guard = self.inner.lock();
    CheckedSpinSyncMutexGuard { parent: self, guard: ManuallyDrop::new(guard) }
  }

  /// Consumes the mutex and returns the inner value.
  pub fn into_inner(self) -> T {
    self.inner.into_inner()
  }
}

/// Guard for [`CheckedSpinSyncMutex`].
pub struct CheckedSpinSyncMutexGuard<'a, T> {
  parent: &'a CheckedSpinSyncMutex<T>,
  guard:  ManuallyDrop<MutexGuard<'a, T>>,
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
