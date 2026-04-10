//! Runtime-selected rwlock surface with a default spin driver.

#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use crate::core::sync::{RwLockDriver, RwLockDriverFactory, SpinSyncRwLock, SpinSyncRwLockFactory};

/// Runtime-selected rwlock surface.
pub struct RuntimeRwLock<T, D = <SpinSyncRwLockFactory as RwLockDriverFactory>::Driver<T>>
where
  D: RwLockDriver<T>, {
  driver: D,
  _pd:    PhantomData<fn() -> T>,
}

impl<T> RuntimeRwLock<T> {
  /// Creates a new runtime-selected rwlock using the default spin driver.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self { driver: SpinSyncRwLock::new(value), _pd: PhantomData }
  }
}

impl<T, D> RuntimeRwLock<T, D>
where
  D: RwLockDriver<T>,
{
  /// Creates a new runtime-selected rwlock using the requested driver.
  #[must_use]
  pub fn new_with_driver(value: T) -> Self {
    Self { driver: D::new(value), _pd: PhantomData }
  }

  /// Acquires a shared read guard.
  pub fn read(&self) -> D::ReadGuard<'_> {
    self.driver.read()
  }

  /// Acquires an exclusive write guard.
  pub fn write(&self) -> D::WriteGuard<'_> {
    self.driver.write()
  }

  /// Consumes the rwlock and returns the inner value.
  pub fn into_inner(self) -> T {
    self.driver.into_inner()
  }
}
