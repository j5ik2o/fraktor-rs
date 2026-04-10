//! Shared wrapper for PlacementCoordinatorCore.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use super::placement_coordinator::PlacementCoordinatorCore;

/// Shared wrapper enabling interior mutability for PlacementCoordinatorCore.
pub struct PlacementCoordinatorShared {
  inner: SharedLock<PlacementCoordinatorCore>,
}

impl PlacementCoordinatorShared {
  /// Wraps a placement coordinator in a shared lock.
  #[must_use]
  pub fn new(coordinator: PlacementCoordinatorCore) -> Self {
    Self { inner: SharedLock::new_with_driver::<SpinSyncMutex<_>>(coordinator) }
  }
}

impl Clone for PlacementCoordinatorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<PlacementCoordinatorCore> for PlacementCoordinatorShared {
  fn with_read<R>(&self, f: impl FnOnce(&PlacementCoordinatorCore) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut PlacementCoordinatorCore) -> R) -> R {
    self.inner.with_write(f)
  }
}
