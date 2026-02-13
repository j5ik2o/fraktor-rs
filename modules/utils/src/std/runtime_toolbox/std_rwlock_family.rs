#![allow(cfg_std_forbid)]

use crate::{core::runtime_toolbox::sync_rwlock_family::SyncRwLockFamily, std::sync_rwlock::StdSyncRwLock};

/// RwLock family backed by [`std::sync::RwLock`].
#[derive(Clone, Copy, Debug, Default)]
pub struct StdRwLockFamily;

impl SyncRwLockFamily for StdRwLockFamily {
  type RwLock<T>
    = StdSyncRwLock<T>
  where
    T: Send + Sync + 'static;

  /// Creates a new std-based read-write lock.
  fn create<T>(value: T) -> Self::RwLock<T>
  where
    T: Send + Sync + 'static, {
    StdSyncRwLock::new(value)
  }
}
