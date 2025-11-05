//! Mutex family backed by [`SpinSyncMutex`], suited for no_std environments.

use crate::{runtime_toolbox::sync_mutex_family::SyncMutexFamily, sync::sync_mutex_like::SpinSyncMutex};

#[cfg(test)]
mod tests;

/// Mutex family backed by [`SpinSyncMutex`], suited for no_std environments.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpinMutexFamily;

impl SyncMutexFamily for SpinMutexFamily {
  type Mutex<T>
    = SpinSyncMutex<T>
  where
    T: Send + 'static;

  fn create<T>(value: T) -> Self::Mutex<T>
  where
    T: Send + 'static, {
    SpinSyncMutex::new(value)
  }
}
