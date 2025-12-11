#![allow(cfg_std_forbid)]

extern crate std;

use core::ops::Deref;
use std::sync::RwLockReadGuard;

#[cfg(test)]
mod tests;

/// Guard returned by [`StdSyncRwLock::read`](crate::std::sync_rwlock::StdSyncRwLock::read).
pub struct StdSyncRwLockReadGuard<'a, T> {
  guard: RwLockReadGuard<'a, T>,
}

impl<'a, T> StdSyncRwLockReadGuard<'a, T> {
  /// Wraps a [`RwLockReadGuard`] into the crate-level guard type.
  #[must_use]
  pub const fn new(guard: RwLockReadGuard<'a, T>) -> Self {
    Self { guard }
  }
}

impl<T> Deref for StdSyncRwLockReadGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}
