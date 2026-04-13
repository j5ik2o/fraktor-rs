//! Re-entry detecting spin-based rwlock for debug/test instrumentation (no_std compatible).

#[cfg(test)]
mod tests;

use core::{
  mem::ManuallyDrop,
  ops::{Deref, DerefMut},
  sync::atomic::{AtomicU8, Ordering},
};

use spin::{RwLockReadGuard, RwLockWriteGuard};

use super::{RwLockDriver, spin_sync_rwlock::SpinSyncRwLock};

/// Lock state constants.
const STATE_FREE: u8 = 0;
const STATE_READ: u8 = 1;
const STATE_WRITE: u8 = 2;

/// Spin-based rwlock with re-entry detection.
///
/// Wraps [`SpinSyncRwLock`] and adds an `AtomicU8` state flag that panics when:
/// - `write()` is called while a write lock is held (write re-entry)
/// - `write()` is called while a read lock is held (read→write upgrade)
/// - `read()` is called while a write lock is held (write→read downgrade attempt)
///
/// `read()` → `read()` re-entry is **not** detected because `spin::RwLock`
/// supports concurrent readers without deadlock.
///
/// This variant does **not** require `std::thread` and works in `no_std`.
pub struct CheckedSpinSyncRwLock<T> {
  inner: SpinSyncRwLock<T>,
  state: AtomicU8,
}

unsafe impl<T: Send> Send for CheckedSpinSyncRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for CheckedSpinSyncRwLock<T> {}

impl<T> CheckedSpinSyncRwLock<T> {
  /// Creates a new checked rwlock.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self { inner: SpinSyncRwLock::new(value), state: AtomicU8::new(STATE_FREE) }
  }

  /// Acquires a shared read guard.
  ///
  /// # Panics
  ///
  /// Panics if a write lock is currently held.
  pub fn read(&self) -> CheckedRwLockReadGuard<'_, T> {
    let prev = self.state.load(Ordering::Acquire);
    assert!(prev != STATE_WRITE, "CheckedSpinSyncRwLock: read lock while write lock held");
    self.state.store(STATE_READ, Ordering::Release);
    let guard = self.inner.read();
    CheckedRwLockReadGuard { parent: self, guard: ManuallyDrop::new(guard) }
  }

  /// Acquires an exclusive write guard.
  ///
  /// # Panics
  ///
  /// Panics if any lock (read or write) is currently held.
  pub fn write(&self) -> CheckedRwLockWriteGuard<'_, T> {
    let prev = self.state.load(Ordering::Acquire);
    assert!(
      prev == STATE_FREE,
      "CheckedSpinSyncRwLock: {}",
      match prev {
        STATE_READ => "write lock while read lock held",
        STATE_WRITE => "re-entrant write lock detected",
        _ => "unexpected lock state",
      }
    );
    self.state.store(STATE_WRITE, Ordering::Release);
    let guard = self.inner.write();
    CheckedRwLockWriteGuard { parent: self, guard: ManuallyDrop::new(guard) }
  }

  /// Consumes the rwlock and returns the inner value.
  pub fn into_inner(self) -> T {
    self.inner.into_inner()
  }
}

/// Read guard for [`CheckedSpinSyncRwLock`].
pub struct CheckedRwLockReadGuard<'a, T> {
  parent: &'a CheckedSpinSyncRwLock<T>,
  guard:  ManuallyDrop<RwLockReadGuard<'a, T>>,
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

/// Write guard for [`CheckedSpinSyncRwLock`].
pub struct CheckedRwLockWriteGuard<'a, T> {
  parent: &'a CheckedSpinSyncRwLock<T>,
  guard:  ManuallyDrop<RwLockWriteGuard<'a, T>>,
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
    unsafe { ManuallyDrop::drop(&mut self.guard) };
    self.parent.state.store(STATE_FREE, Ordering::Release);
  }
}

impl<T> RwLockDriver<T> for CheckedSpinSyncRwLock<T> {
  type ReadGuard<'a>
    = CheckedRwLockReadGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type WriteGuard<'a>
    = CheckedRwLockWriteGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self {
    Self::new(value)
  }

  fn read(&self) -> Self::ReadGuard<'_> {
    CheckedSpinSyncRwLock::read(self)
  }

  fn write(&self) -> Self::WriteGuard<'_> {
    CheckedSpinSyncRwLock::write(self)
  }

  fn into_inner(self) -> T {
    CheckedSpinSyncRwLock::into_inner(self)
  }
}
