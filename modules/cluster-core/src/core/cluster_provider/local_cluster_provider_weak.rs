//! Weak wrapper for LocalClusterProvider shared handles.

use fraktor_utils_core_rs::core::sync::WeakSharedLock;

use super::{LocalClusterProvider, LocalClusterProviderShared};

/// Weak counterpart of [`LocalClusterProviderShared`].
pub struct LocalClusterProviderWeak {
  pub(crate) inner: WeakSharedLock<LocalClusterProvider>,
}

impl LocalClusterProviderWeak {
  /// Upgrades the weak provider handle if the provider is still alive.
  #[must_use]
  pub fn upgrade(&self) -> Option<LocalClusterProviderShared> {
    self.inner.upgrade().map(|inner| LocalClusterProviderShared { inner })
  }
}

impl Clone for LocalClusterProviderWeak {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
