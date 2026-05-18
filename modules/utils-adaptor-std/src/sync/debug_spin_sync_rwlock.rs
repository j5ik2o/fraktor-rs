#[cfg(test)]
#[path = "debug_spin_sync_rwlock_test.rs"]
mod tests;

use fraktor_utils_core_rs::sync::RwLockDriver;
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Debug spin rwlock. V1 keeps spin semantics and provides a parallel driver
/// family for tests that need rwlock selection.
pub struct DebugSpinSyncRwLock<T>(RwLock<T>);

unsafe impl<T: Send> Send for DebugSpinSyncRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for DebugSpinSyncRwLock<T> {}

impl<T> DebugSpinSyncRwLock<T> {
  /// Creates a new debug rwlock.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self(RwLock::new(value))
  }

  /// Acquires a shared read guard.
  pub fn read(&self) -> RwLockReadGuard<'_, T> {
    self.0.read()
  }

  /// Acquires an exclusive write guard.
  pub fn write(&self) -> RwLockWriteGuard<'_, T> {
    self.0.write()
  }

  /// Consumes the rwlock and returns the inner value.
  pub fn into_inner(self) -> T {
    self.0.into_inner()
  }
}

impl<T> RwLockDriver<T> for DebugSpinSyncRwLock<T> {
  type ReadGuard<'a>
    = RwLockReadGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type WriteGuard<'a>
    = RwLockWriteGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self {
    Self::new(value)
  }

  fn read(&self) -> Self::ReadGuard<'_> {
    DebugSpinSyncRwLock::read(self)
  }

  fn write(&self) -> Self::WriteGuard<'_> {
    DebugSpinSyncRwLock::write(self)
  }

  fn into_inner(self) -> T {
    DebugSpinSyncRwLock::into_inner(self)
  }
}
