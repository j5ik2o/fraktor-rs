//! Shared wrapper for endpoint association coordinator.

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeRwLock, SharedAccess, sync_rwlock_like::SyncRwLockLike};

use super::coordinator::EndpointAssociationCoordinator;

/// Shared wrapper for [`EndpointAssociationCoordinator`] enabling interior mutability.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying [`EndpointAssociationCoordinator`], allowing safe
/// concurrent access from multiple owners.
pub struct EndpointAssociationCoordinatorShared {
  inner: ArcShared<RuntimeRwLock<EndpointAssociationCoordinator>>,
}

impl Clone for EndpointAssociationCoordinatorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl Default for EndpointAssociationCoordinatorShared {
  fn default() -> Self {
    Self::new()
  }
}

impl EndpointAssociationCoordinatorShared {
  /// Creates a new shared endpoint association coordinator instance.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(RuntimeRwLock::new(EndpointAssociationCoordinator::new())) }
  }
}

impl SharedAccess<EndpointAssociationCoordinator> for EndpointAssociationCoordinatorShared {
  fn with_read<R>(&self, f: impl FnOnce(&EndpointAssociationCoordinator) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EndpointAssociationCoordinator) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
