use fraktor_utils_core_rs::core::sync::RwLockDriverFactory;

use super::std_sync_rwlock::StdSyncRwLock;

/// Factory for [`StdSyncRwLock`](super::StdSyncRwLock).
pub struct StdSyncRwLockFactory;

impl RwLockDriverFactory for StdSyncRwLockFactory {
  type Driver<T> = StdSyncRwLock<T>;
}
