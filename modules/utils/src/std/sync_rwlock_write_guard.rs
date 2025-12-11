#![allow(cfg_std_forbid)]

extern crate std;

use core::ops::{Deref, DerefMut};
use std::sync::RwLockWriteGuard;

#[cfg(test)]
mod tests;

/// Guard returned by [`StdSyncRwLock::write`](crate::std::sync_rwlock::StdSyncRwLock::write).
pub struct StdSyncRwLockWriteGuard<'a, T> {
  guard: RwLockWriteGuard<'a, T>,
}

impl<'a, T> StdSyncRwLockWriteGuard<'a, T> {
  /// Wraps a [`RwLockWriteGuard`] into the crate-level guard type.
  #[must_use]
  pub const fn new(guard: RwLockWriteGuard<'a, T>) -> Self {
    Self { guard }
  }
}

impl<T> Deref for StdSyncRwLockWriteGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> DerefMut for StdSyncRwLockWriteGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}
