//! Shared wrapper for MembershipCoordinator.

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeMutex, RuntimeToolbox},
  sync::{ArcShared, SharedAccess},
};

use super::MembershipCoordinatorGeneric;

/// Shared wrapper enabling interior mutability for MembershipCoordinator.
pub struct MembershipCoordinatorSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<RuntimeMutex<MembershipCoordinatorGeneric<TB>>>,
}

impl<TB: RuntimeToolbox + 'static> MembershipCoordinatorSharedGeneric<TB> {
  /// Wraps a membership coordinator in a shared mutex.
  #[must_use]
  pub fn new(coordinator: MembershipCoordinatorGeneric<TB>) -> Self {
    let inner = RuntimeMutex::new(coordinator);
    Self { inner: ArcShared::new(inner) }
  }

  /// Creates from an existing shared inner.
  #[must_use]
  pub const fn from_inner(inner: ArcShared<RuntimeMutex<MembershipCoordinatorGeneric<TB>>>) -> Self {
    Self { inner }
  }

  /// Returns the inner shared handle.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<MembershipCoordinatorGeneric<TB>>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for MembershipCoordinatorSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<MembershipCoordinatorGeneric<TB>>
  for MembershipCoordinatorSharedGeneric<TB>
{
  fn with_read<R>(&self, f: impl FnOnce(&MembershipCoordinatorGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MembershipCoordinatorGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
