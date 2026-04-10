//! Public lock-factory seam used by stable actor runtime wrappers.

use fraktor_utils_core_rs::core::sync::SharedLock;

/// Factory for stable actor-runtime shared wrappers.
pub trait ActorRuntimeLockFactory {
  /// Creates a shared lock for runtime-owned actor state.
  fn create_lock<T>(&self, value: T) -> SharedLock<T>
  where
    T: Send + 'static;
}
