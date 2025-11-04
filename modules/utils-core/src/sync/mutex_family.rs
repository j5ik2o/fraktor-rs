//! Mutex family abstraction for runtime injection.

#[cfg(test)]
mod tests;

use crate::sync::sync_mutex_like::SyncMutexLike;

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
