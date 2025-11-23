#![allow(cfg_std_forbid)]

extern crate std;

use core::ops::{Deref, DerefMut};
use std::sync::MutexGuard;

#[cfg(test)]
mod tests;

/// Guard returned by [`StdSyncMutex`](crate::StdSyncMutex).
pub struct StdSyncMutexGuard<'a, T> {
  /// Underlying mutex guard.
  pub guard: MutexGuard<'a, T>,
}

impl<T> Deref for StdSyncMutexGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> DerefMut for StdSyncMutexGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}
