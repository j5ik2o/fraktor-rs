//! Shared wrapper for MembershipCoordinator.

use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeMutex,
  sync::{ArcShared, SharedAccess},
};

use super::MembershipCoordinator;

/// Shared wrapper enabling interior mutability for MembershipCoordinator.
pub struct MembershipCoordinatorShared {
  inner: ArcShared<RuntimeMutex<MembershipCoordinator>>,
}

impl MembershipCoordinatorShared {
  /// Wraps a membership coordinator in a shared mutex.
  #[must_use]
  pub fn new(coordinator: MembershipCoordinator) -> Self {
    let inner = RuntimeMutex::new(coordinator);
    Self { inner: ArcShared::new(inner) }
  }

  /// Creates from an existing shared inner.
  #[must_use]
  pub const fn from_inner(inner: ArcShared<RuntimeMutex<MembershipCoordinator>>) -> Self {
    Self { inner }
  }

  /// Returns the inner shared handle.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<MembershipCoordinator>> {
    self.inner.clone()
  }
}

impl Clone for MembershipCoordinatorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<MembershipCoordinator> for MembershipCoordinatorShared {
  fn with_read<R>(&self, f: impl FnOnce(&MembershipCoordinator) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MembershipCoordinator) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
