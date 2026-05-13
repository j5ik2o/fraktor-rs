use fraktor_utils_core_rs::sync::LockDriverFactory;

use super::debug_spin_sync_mutex::DebugSpinSyncMutex;

/// Factory for [`DebugSpinSyncMutex`](super::DebugSpinSyncMutex).
pub struct DebugSpinSyncFactory;

impl LockDriverFactory for DebugSpinSyncFactory {
  type Driver<T> = DebugSpinSyncMutex<T>;
}
