//! Factory contract for materializing shared actor futures.

use crate::core::kernel::util::futures::{ActorFuture, ActorFutureShared};

/// Materializes [`ActorFutureShared`] using the runtime-selected lock family.
pub trait ActorFutureSharedFactory<T>: Send + Sync
where
  T: Send + 'static, {
  /// Wraps the provided future with the runtime-selected shared representation.
  fn create(&self, future: ActorFuture<T>) -> ActorFutureShared<T>;
}
