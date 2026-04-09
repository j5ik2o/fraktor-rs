use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use fraktor_utils_core_rs::core::sync::{RwLockDriver, RwLockDriverFactory};

/// Standard-library-backed rwlock driver.
pub struct StdSyncRwLock<T>(RwLock<T>);

impl<T> StdSyncRwLock<T> {
  /// Creates a new std rwlock.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self(RwLock::new(value))
  }

  /// Acquires a shared read guard, absorbing poison.
  pub fn read(&self) -> RwLockReadGuard<'_, T> {
    self.0.read().unwrap_or_else(PoisonError::into_inner)
  }

  /// Acquires an exclusive write guard, absorbing poison.
  pub fn write(&self) -> RwLockWriteGuard<'_, T> {
    self.0.write().unwrap_or_else(PoisonError::into_inner)
  }

  /// Consumes the rwlock and returns the inner value, absorbing poison if needed.
  pub fn into_inner(self) -> T {
    self.0.into_inner().unwrap_or_else(PoisonError::into_inner)
  }
}

impl<T> RwLockDriver<T> for StdSyncRwLock<T> {
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
    self.read()
  }

  fn write(&self) -> Self::WriteGuard<'_> {
    self.write()
  }

  fn into_inner(self) -> T {
    self.into_inner()
  }
}

/// Factory for [`StdSyncRwLock`].
pub struct StdSyncRwLockFactory;

impl RwLockDriverFactory for StdSyncRwLockFactory {
  type Driver<T> = StdSyncRwLock<T>;
}

#[cfg(test)]
mod tests {
  use super::StdSyncRwLock;

  #[test]
  fn reads_and_writes_value() {
    let rwlock = StdSyncRwLock::new(2_u32);
    assert_eq!(*rwlock.read(), 2);
    *rwlock.write() = 4;
    assert_eq!(*rwlock.read(), 4);
  }
}
