use cellactor_utils_core_rs::runtime_toolbox::SyncMutexFamily;

use crate::StdSyncMutex;

/// Mutex family backed by [`std::sync::Mutex`].
#[derive(Clone, Copy, Debug, Default)]
pub struct StdMutexFamily;

impl SyncMutexFamily for StdMutexFamily {
  type Mutex<T>
    = StdSyncMutex<T>
  where
    T: Send + 'static;

  fn create<T>(value: T) -> Self::Mutex<T>
  where
    T: Send + 'static, {
    StdSyncMutex::new(value)
  }
}
