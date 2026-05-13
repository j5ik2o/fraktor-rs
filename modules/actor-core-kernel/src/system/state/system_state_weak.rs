//! Weak reference wrapper for system state.

use fraktor_utils_core_rs::sync::WeakSharedRwLock;

use super::{system_state::SystemState, system_state_shared::SystemStateShared};

/// Weak reference wrapper for [`SystemState`].
///
/// This wrapper avoids circular reference issues between system state and actor cells.
pub struct SystemStateWeak {
  pub(crate) inner: WeakSharedRwLock<SystemState>,
}

impl Clone for SystemStateWeak {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SystemStateWeak {
  /// Attempts to upgrade the weak reference to a strong reference.
  ///
  /// Returns `None` if the system state has been dropped.
  #[must_use]
  pub fn upgrade(&self) -> Option<SystemStateShared> {
    self.inner.upgrade().map(SystemStateShared::from_shared_rw_lock)
  }
}
