use cellactor_utils_core_rs::sync::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex};

use crate::sync_mutex::StdSyncMutex;

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

/// Toolbox for std environments, backed by [`StdMutexFamily`].
#[derive(Clone, Copy, Debug, Default)]
pub struct StdToolbox;

impl RuntimeToolbox for StdToolbox {
  type MutexFamily = StdMutexFamily;
}

/// Convenience alias for the default std mutex.
pub type StdMutex<T> = ToolboxMutex<T, StdToolbox>;
