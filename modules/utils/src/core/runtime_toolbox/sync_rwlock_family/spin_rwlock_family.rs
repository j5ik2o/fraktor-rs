//! RwLock family backed by [`SpinSyncRwLock`], suited for no_std environments.

use crate::core::{runtime_toolbox::sync_rwlock_family::SyncRwLockFamily, sync::sync_rwlock_like::SpinSyncRwLock};

#[cfg(test)]
mod tests;

/// RwLock family backed by [`SpinSyncRwLock`], suited for no_std environments.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpinRwLockFamily;

impl SyncRwLockFamily for SpinRwLockFamily {
  type RwLock<T>
    = SpinSyncRwLock<T>
  where
    T: Send + Sync + 'static;

  /// Creates a new spin-based read-write lock.
  fn create<T>(value: T) -> Self::RwLock<T>
  where
    T: Send + Sync + 'static, {
    SpinSyncRwLock::new(value)
  }
}
