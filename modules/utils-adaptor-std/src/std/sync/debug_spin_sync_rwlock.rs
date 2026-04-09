use fraktor_utils_core_rs::core::sync::{RwLockDriver, RwLockDriverFactory};

/// Debug spin rwlock. V1 keeps spin semantics and provides a parallel driver
/// family for tests that need rwlock selection.
pub struct DebugSpinSyncRwLock<T>(spin::RwLock<T>);

unsafe impl<T: Send> Send for DebugSpinSyncRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for DebugSpinSyncRwLock<T> {}

impl<T> DebugSpinSyncRwLock<T> {
  /// Creates a new debug rwlock.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self(spin::RwLock::new(value))
  }

  /// Acquires a shared read guard.
  pub fn read(&self) -> spin::RwLockReadGuard<'_, T> {
    self.0.read()
  }

  /// Acquires an exclusive write guard.
  pub fn write(&self) -> spin::RwLockWriteGuard<'_, T> {
    self.0.write()
  }

  /// Consumes the rwlock and returns the inner value.
  pub fn into_inner(self) -> T {
    self.0.into_inner()
  }
}

impl<T> RwLockDriver<T> for DebugSpinSyncRwLock<T> {
  type ReadGuard<'a>
    = spin::RwLockReadGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  type WriteGuard<'a>
    = spin::RwLockWriteGuard<'a, T>
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

/// Factory for [`DebugSpinSyncRwLock`].
pub struct DebugSpinSyncRwLockFactory;

impl RwLockDriverFactory for DebugSpinSyncRwLockFactory {
  type Driver<T> = DebugSpinSyncRwLock<T>;
}

#[cfg(test)]
mod tests {
  use super::DebugSpinSyncRwLock;

  #[test]
  fn reads_and_writes_value() {
    let rwlock = DebugSpinSyncRwLock::new(5_u32);
    assert_eq!(*rwlock.read(), 5);
    *rwlock.write() = 8;
    assert_eq!(*rwlock.read(), 8);
  }
}
