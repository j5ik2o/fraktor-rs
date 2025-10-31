#![cfg(feature = "std")]

use core::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};

use crate::sync::sync_mutex_like::SyncMutexLike;

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

/// Guard returned by [`StdSyncMutex`].
pub struct StdSyncMutexGuard<'a, T> {
  guard: MutexGuard<'a, T>,
}

impl<'a, T> Deref for StdSyncMutexGuard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &*self.guard
  }
}

#[cfg(feature = "std")]
impl<'a, T> DerefMut for StdSyncMutexGuard<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut *self.guard
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
