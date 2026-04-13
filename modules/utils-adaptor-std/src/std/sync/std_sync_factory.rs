use fraktor_utils_core_rs::core::sync::LockDriverFactory;

use super::StdSyncMutex;

/// Factory for [`StdSyncMutex`](super::StdSyncMutex).
pub struct StdSyncFactory;

impl LockDriverFactory for StdSyncFactory {
  type Driver<T> = StdSyncMutex<T>;
}
