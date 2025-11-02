//! Mutex family abstraction for runtime injection.

use crate::sync::sync_mutex_like::{SpinSyncMutex, SyncMutexLike};

/// Provides a constructor for mutex implementations used by the runtime.
pub trait SyncMutexFamily {
  /// Concrete mutex type produced by this family.
  type Mutex<T>: SyncMutexLike<T> + Send + 'static
  where
    T: Send + 'static;

  /// Creates a new mutex protecting the given value.
  fn create<T>(value: T) -> Self::Mutex<T>
  where
    T: Send + 'static;
}

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

#[cfg(test)]
mod tests;
