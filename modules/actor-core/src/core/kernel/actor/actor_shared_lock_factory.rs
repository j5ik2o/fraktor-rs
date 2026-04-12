//! Factory contract for actor instance shared locks.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::SharedLock;

use super::Actor;

/// Materializes the shared lock used by actor runtime instances.
pub trait ActorSharedLockFactory: Send + Sync {
  /// Creates a shared actor lock.
  fn create(&self, actor: Box<dyn Actor + Send>) -> SharedLock<Box<dyn Actor + Send>>;
}
