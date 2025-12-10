//! Weak reference wrapper for system state.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::WeakShared,
};

use super::{SystemStateGeneric, SystemStateSharedGeneric};

/// Weak reference wrapper for [`SystemStateGeneric`].
///
/// This wrapper avoids circular reference issues between system state and actor cells.
pub struct SystemStateWeakGeneric<TB: RuntimeToolbox + 'static> {
  pub(crate) inner: WeakShared<SystemStateGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for SystemStateWeakGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SystemStateWeakGeneric<TB> {
  /// Attempts to upgrade the weak reference to a strong reference.
  ///
  /// Returns `None` if the system state has been dropped.
  #[must_use]
  pub fn upgrade(&self) -> Option<SystemStateSharedGeneric<TB>> {
    self.inner.upgrade().map(|inner| SystemStateSharedGeneric::from_arc_shared(inner))
  }
}

/// Type alias with the default `NoStdToolbox`.
#[allow(dead_code)]
pub(crate) type SystemStateWeak = SystemStateWeakGeneric<NoStdToolbox>;
