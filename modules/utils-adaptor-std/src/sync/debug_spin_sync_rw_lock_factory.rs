use fraktor_utils_core_rs::sync::RwLockDriverFactory;

use super::debug_spin_sync_rwlock::DebugSpinSyncRwLock;

/// Factory for [`DebugSpinSyncRwLock`](super::DebugSpinSyncRwLock).
pub struct DebugSpinSyncRwLockFactory;

impl RwLockDriverFactory for DebugSpinSyncRwLockFactory {
  type Driver<T> = DebugSpinSyncRwLock<T>;
}
