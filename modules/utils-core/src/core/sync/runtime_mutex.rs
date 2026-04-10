//! Runtime-selected mutex surface with a default spin driver.

#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use crate::core::sync::{LockDriver, LockDriverFactory, SpinSyncFactory, SpinSyncMutex};

/// Runtime-selected mutex surface.
pub struct RuntimeMutex<T, D = <SpinSyncFactory as LockDriverFactory>::Driver<T>>
where
  D: LockDriver<T>, {
  driver: D,
  _pd:    PhantomData<fn() -> T>,
}

impl<T> RuntimeMutex<T> {
  /// Creates a new runtime-selected mutex using the default spin driver.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self { driver: SpinSyncMutex::new(value), _pd: PhantomData }
  }
}

impl<T, D> RuntimeMutex<T, D>
where
  D: LockDriver<T>,
{
  /// Creates a new runtime-selected mutex using the requested driver.
  #[must_use]
  pub fn new_with_driver(value: T) -> Self {
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
