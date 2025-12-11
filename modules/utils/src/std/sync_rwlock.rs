#![allow(cfg_std_forbid)]
#![allow(clippy::disallowed_types)]

extern crate std;

use std::sync::RwLock;

use super::{sync_rwlock_read_guard::StdSyncRwLockReadGuard, sync_rwlock_write_guard::StdSyncRwLockWriteGuard};
use crate::core::sync::sync_rwlock_like::SyncRwLockLike;

#[cfg(test)]
mod tests;

/// Read-write lock wrapper backed by [`std::sync::RwLock`].
pub struct StdSyncRwLock<T>(RwLock<T>);

impl<T> StdSyncRwLock<T> {
  /// Creates a new read-write lock-backed value.
  #[must_use]
  #[allow(clippy::disallowed_types)]
  pub const fn new(value: T) -> Self {
    Self(RwLock::new(value))
  }

  /// Consumes the lock and returns the inner value.
  pub fn into_inner(self) -> T {
    match self.0.into_inner() {
      | Ok(value) => value,
      | Err(poisoned) => poisoned.into_inner(),
    }
  }
}

impl<T> SyncRwLockLike<T> for StdSyncRwLock<T> {
  type ReadGuard<'a>
    = StdSyncRwLockReadGuard<'a, T>
  where
    T: 'a;
  type WriteGuard<'a>
    = StdSyncRwLockWriteGuard<'a, T>
  where
    T: 'a;

  fn new(value: T) -> Self {
    StdSyncRwLock::new(value)
  }

  fn into_inner(self) -> T {
    StdSyncRwLock::into_inner(self)
  }

  fn read(&self) -> Self::ReadGuard<'_> {
    match self.0.read() {
      | Ok(guard) => StdSyncRwLockReadGuard::new(guard),
      | Err(poisoned) => StdSyncRwLockReadGuard::new(poisoned.into_inner()),
    }
  }

  fn write(&self) -> Self::WriteGuard<'_> {
    match self.0.write() {
      | Ok(guard) => StdSyncRwLockWriteGuard::new(guard),
      | Err(poisoned) => StdSyncRwLockWriteGuard::new(poisoned.into_inner()),
    }
  }
}
