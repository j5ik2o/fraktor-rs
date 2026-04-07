//! Shared wrapper for PlacementCoordinatorCore.

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::placement_coordinator::PlacementCoordinatorCore;

/// Shared wrapper enabling interior mutability for PlacementCoordinatorCore.
pub struct PlacementCoordinatorShared {
  inner: ArcShared<RuntimeMutex<PlacementCoordinatorCore>>,
}

impl PlacementCoordinatorShared {
  /// Wraps a placement coordinator in a shared mutex.
  #[must_use]
  pub fn new(coordinator: PlacementCoordinatorCore) -> Self {
    let inner = RuntimeMutex::new(coordinator);
    Self { inner: ArcShared::new(inner) }
  }

  /// Creates from an existing shared inner.
  #[must_use]
  pub const fn from_inner(inner: ArcShared<RuntimeMutex<PlacementCoordinatorCore>>) -> Self {
    Self { inner }
  }

  /// Returns the inner shared handle.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<PlacementCoordinatorCore>> {
    self.inner.clone()
  }
}

impl Clone for PlacementCoordinatorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<PlacementCoordinatorCore> for PlacementCoordinatorShared {
  fn with_read<R>(&self, f: impl FnOnce(&PlacementCoordinatorCore) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut PlacementCoordinatorCore) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
