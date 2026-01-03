//! Shared wrapper for endpoint association coordinator.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::coordinator::EndpointAssociationCoordinator;

/// Shared wrapper for [`EndpointAssociationCoordinator`] enabling interior mutability.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying [`EndpointAssociationCoordinator`], allowing safe
/// concurrent access from multiple owners.
pub struct EndpointAssociationCoordinatorSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxRwLock<EndpointAssociationCoordinator, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for EndpointAssociationCoordinatorSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EndpointAssociationCoordinatorSharedGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> EndpointAssociationCoordinatorSharedGeneric<TB> {
  /// Creates a new shared endpoint association coordinator instance.
  #[must_use]
  pub fn new() -> Self {
    Self {
      inner: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(EndpointAssociationCoordinator::new())),
    }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<EndpointAssociationCoordinator>
  for EndpointAssociationCoordinatorSharedGeneric<TB>
{
  fn with_read<R>(&self, f: impl FnOnce(&EndpointAssociationCoordinator) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EndpointAssociationCoordinator) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}

/// Type alias for [`EndpointAssociationCoordinatorSharedGeneric`] using the default
/// [`NoStdToolbox`].
pub type EndpointAssociationCoordinatorShared = EndpointAssociationCoordinatorSharedGeneric<NoStdToolbox>;
