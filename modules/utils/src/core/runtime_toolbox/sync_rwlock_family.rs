//! RwLock family abstraction for runtime injection.

mod spin_rwlock_family;
#[cfg(test)]
mod tests;

pub use spin_rwlock_family::SpinRwLockFamily;

use crate::core::sync::sync_rwlock_like::SyncRwLockLike;

/// Provides a constructor for read-write lock implementations used by the runtime.
pub trait SyncRwLockFamily {
  /// Concrete read-write lock type produced by this family.
  type RwLock<T>: SyncRwLockLike<T> + Send + Sync + 'static
  where
    T: Send + Sync + 'static;

  /// Creates a new read-write lock protecting the given value.
  fn create<T>(value: T) -> Self::RwLock<T>
  where
    T: Send + Sync + 'static;
}
