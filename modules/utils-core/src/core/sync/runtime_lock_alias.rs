//! Runtime-selected lock type aliases shared across the crate.

#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use crate::core::sync::{
  LockDriver, RwLockDriver, SpinSyncFactory, SpinSyncMutex, SpinSyncRwLockFactory,
};

/// Runtime-selected mutex surface with a default no-std driver.
pub struct RuntimeMutex<T, D = <SpinSyncFactory as crate::core::sync::LockDriverFactory>::Driver<T>>
where
  D: LockDriver<T>,
{
  driver: D,
  _pd:    PhantomData<T>,
}

impl<T, D> RuntimeMutex<T, D>
where
  D: LockDriver<T>,
{
  /// Creates a new runtime-selected mutex.
  #[must_use]
  pub fn new(value: T) -> Self {
    Self { driver: D::new(value), _pd: PhantomData }
  }

  /// Acquires the mutex guard.
  pub fn lock(&self) -> D::Guard<'_> {
    self.driver.lock()
  }

  /// Consumes the mutex and returns the inner value.
  pub fn into_inner(self) -> T {
    self.driver.into_inner()
  }
}

/// Runtime-selected rwlock surface with a default no-std driver.
pub struct RuntimeRwLock<T, D = <SpinSyncRwLockFactory as crate::core::sync::RwLockDriverFactory>::Driver<T>>
where
  D: RwLockDriver<T>,
{
  driver: D,
  _pd:    PhantomData<T>,
}

impl<T, D> RuntimeRwLock<T, D>
where
  D: RwLockDriver<T>,
{
  /// Creates a new runtime-selected rwlock.
  #[must_use]
  pub fn new(value: T) -> Self {
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

/// No-std mutex alias.
pub type NoStdMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>;
