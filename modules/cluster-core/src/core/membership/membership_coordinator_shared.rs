//! Shared wrapper for MembershipCoordinator.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, DefaultMutex};

use super::MembershipCoordinator;

/// Shared wrapper enabling interior mutability for MembershipCoordinator.
pub struct MembershipCoordinatorShared {
  inner: SharedLock<MembershipCoordinator>,
}

impl MembershipCoordinatorShared {
  /// Wraps a membership coordinator in a shared lock.
  #[must_use]
  pub fn new(coordinator: MembershipCoordinator) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(coordinator) }
  }
}

impl Clone for MembershipCoordinatorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<MembershipCoordinator> for MembershipCoordinatorShared {
  fn with_read<R>(&self, f: impl FnOnce(&MembershipCoordinator) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MembershipCoordinator) -> R) -> R {
    self.inner.with_write(f)
  }
}
