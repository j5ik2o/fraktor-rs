#[cfg(test)]
#[path = "std_sync_mutex_test.rs"]
mod tests;

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::sync::LockDriver;

/// Standard-library-backed mutex driver.
///
/// Wraps [`std::sync::Mutex`] and absorbs poison errors so that a panicked
/// thread does not permanently lock out other threads.
pub struct StdSyncMutex<T>(Mutex<T>);

impl<T> StdSyncMutex<T> {
  /// Creates a new std mutex.
  #[must_use]
  pub const fn new(value: T) -> Self {
    Self(Mutex::new(value))
  }

  /// Locks the mutex, absorbing poison by taking the inner guard.
  pub fn lock(&self) -> MutexGuard<'_, T> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Consumes the mutex and returns the inner value, absorbing poison if needed.
  pub fn into_inner(self) -> T {
    self.0.into_inner().unwrap_or_else(PoisonError::into_inner)
  }
}

impl<T> LockDriver<T> for StdSyncMutex<T> {
  type Guard<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self {
    Self::new(value)
  }

  fn lock(&self) -> Self::Guard<'_> {
    StdSyncMutex::lock(self)
  }

  fn into_inner(self) -> T {
    StdSyncMutex::into_inner(self)
  }
}
