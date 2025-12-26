//! Placement lock trait used by placement coordination.

use alloc::string::String;
use core::future::Future;

use crate::core::{GrainKey, PlacementLease, PlacementLockError};

/// Exclusive lock used by placement coordination.
pub trait PlacementLock {
  /// Future returned by [`PlacementLock::try_acquire`].
  type TryAcquireFuture<'a>: Future<Output = Result<PlacementLease, PlacementLockError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by [`PlacementLock::release`].
  type ReleaseFuture<'a>: Future<Output = Result<(), PlacementLockError>> + Send + 'a
  where
    Self: 'a;

  /// Attempts to acquire a lock for the given grain key.
  fn try_acquire<'a>(&'a mut self, key: GrainKey, owner: String, now: u64) -> Self::TryAcquireFuture<'a>;

  /// Releases a previously acquired lease.
  fn release<'a>(&'a mut self, lease: PlacementLease) -> Self::ReleaseFuture<'a>;
}
