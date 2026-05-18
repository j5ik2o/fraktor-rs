use fraktor_utils_core_rs::sync::LockDriverFactory;

use super::std_sync_mutex::StdSyncMutex;

/// Factory for [`StdSyncMutex`](super::StdSyncMutex).
pub struct StdSyncFactory;

impl LockDriverFactory for StdSyncFactory {
  type Driver<T> = StdSyncMutex<T>;
}
