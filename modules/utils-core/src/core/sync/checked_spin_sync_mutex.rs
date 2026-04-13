//! Re-entry detecting spin-based mutex for debug/test instrumentation.
//!
//! Requires the `debug-locks` feature (which implies `std`) so that
//! `std::thread::current().id()` can distinguish same-thread re-entry
//! from legitimate cross-thread contention.

#[cfg(test)]
mod tests;

use core::mem::ManuallyDrop;

use std::sync::Mutex;
use std::thread;

use super::{LockDriver, checked_spin_sync_mutex_guard::CheckedSpinSyncMutexGuard, spin_sync_mutex::SpinSyncMutex};

/// Spin-based mutex with thread-aware re-entry detection.
///
/// Wraps [`SpinSyncMutex`] and records the owning thread ID. If `lock()`
/// is called from the same thread that already holds the lock, it panics
/// immediately instead of deadlocking.
pub struct CheckedSpinSyncMutex<T> {
  pub(super) inner: SpinSyncMutex<T>,
  /// `None` = unlocked, `Some(id)` = thread `id` holds the lock.
  pub(super) owner: Mutex<Option<thread::ThreadId>>,
}

unsafe impl<T: Send> Send for CheckedSpinSyncMutex<T> {}
unsafe impl<T: Send> Sync for CheckedSpinSyncMutex<T> {}

impl<T> CheckedSpinSyncMutex<T> {
  /// Creates a new checked mutex.
  #[must_use]
  pub fn new(value: T) -> Self {
    Self { inner: SpinSyncMutex::new(value), owner: Mutex::new(None) }
  }

  /// Acquires the mutex, panicking on same-thread re-entry.
  ///
  /// # Panics
  ///
  /// Panics if the calling thread already holds this lock.
  pub fn lock(&self) -> CheckedSpinSyncMutexGuard<'_, T> {
    let current = thread::current().id();
    {
      let owner = self.owner.lock().unwrap_or_else(|e| e.into_inner());
      if *owner == Some(current) {
        panic!("CheckedSpinSyncMutex: re-entrant lock detected (thread {:?})", current);
      }
    }
    // Not a re-entry — wait for the real lock.
    let guard = self.inner.lock();
    *self.owner.lock().unwrap_or_else(|e| e.into_inner()) = Some(current);
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
