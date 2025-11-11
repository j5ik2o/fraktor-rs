use std::sync::Mutex;

use fraktor_utils_core_rs::sync::sync_mutex_like::SyncMutexLike;

use crate::sync_mutex_guard::StdSyncMutexGuard;

#[cfg(test)]
mod tests;

/// Mutex wrapper backed by [`std::sync::Mutex`].
pub struct StdSyncMutex<T>(Mutex<T>);

impl<T> StdSyncMutex<T> {
  /// Creates a new mutex-backed value.
  #[must_use]
  pub fn new(value: T) -> Self {
    Self(Mutex::new(value))
  }

  /// Consumes the mutex and returns the inner value.
  pub fn into_inner(self) -> T {
    match self.0.into_inner() {
      | Ok(value) => value,
      | Err(poisoned) => poisoned.into_inner(),
    }
  }

  /// Locks the mutex and returns the guard.
  pub fn lock(&self) -> StdSyncMutexGuard<'_, T> {
    match self.0.lock() {
      | Ok(guard) => StdSyncMutexGuard { guard },
      | Err(poisoned) => StdSyncMutexGuard { guard: poisoned.into_inner() },
    }
  }
}

impl<T> SyncMutexLike<T> for StdSyncMutex<T> {
  type Guard<'a>
    = StdSyncMutexGuard<'a, T>
  where
    T: 'a;

  fn new(value: T) -> Self {
    StdSyncMutex::new(value)
  }

  fn into_inner(self) -> T {
    StdSyncMutex::into_inner(self)
  }

  fn lock(&self) -> Self::Guard<'_> {
    StdSyncMutex::lock(self)
  }
}
