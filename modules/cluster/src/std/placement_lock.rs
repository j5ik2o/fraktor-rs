//! Placement lock trait for std implementations.

use std::{boxed::Box, future::Future, pin::Pin};

use crate::core::{GrainKey, PlacementLease, PlacementLockError};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Exclusive lock used by placement coordination.
pub trait PlacementLock {
  /// Attempts to acquire a lock for the given grain key.
  fn try_acquire<'a>(
    &'a mut self,
    key: &'a GrainKey,
    owner: &'a str,
    now: u64,
  ) -> BoxFuture<'a, Result<PlacementLease, PlacementLockError>>;

  /// Releases a previously acquired lease.
  fn release<'a>(&'a mut self, lease: PlacementLease) -> BoxFuture<'a, Result<(), PlacementLockError>>;
}
