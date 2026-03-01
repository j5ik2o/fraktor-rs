//! Shared wrapper for PlacementCoordinatorCore.

use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeMutex, RuntimeToolbox},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::placement_coordinator::PlacementCoordinatorCore;

/// Shared wrapper enabling interior mutability for PlacementCoordinatorCore.
pub struct PlacementCoordinatorSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner:   ArcShared<RuntimeMutex<PlacementCoordinatorCore>>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> PlacementCoordinatorSharedGeneric<TB> {
  /// Wraps a placement coordinator in a shared mutex.
  #[must_use]
  pub fn new(coordinator: PlacementCoordinatorCore) -> Self {
    let inner = RuntimeMutex::new(coordinator);
    Self { inner: ArcShared::new(inner), _marker: PhantomData }
  }

  /// Creates from an existing shared inner.
  #[must_use]
  pub const fn from_inner(inner: ArcShared<RuntimeMutex<PlacementCoordinatorCore>>) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns the inner shared handle.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<PlacementCoordinatorCore>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for PlacementCoordinatorSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<PlacementCoordinatorCore> for PlacementCoordinatorSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&PlacementCoordinatorCore) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut PlacementCoordinatorCore) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
