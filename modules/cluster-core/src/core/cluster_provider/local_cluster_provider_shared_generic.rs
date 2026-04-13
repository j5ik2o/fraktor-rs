//! Shared wrapper for LocalClusterProvider implementations.

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::LocalClusterProvider;

/// Shared wrapper for [`LocalClusterProvider`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying provider, allowing safe
/// concurrent access from multiple owners.
pub struct LocalClusterProviderShared {
  inner: SharedLock<LocalClusterProvider>,
}

impl LocalClusterProviderShared {
  /// Creates a new shared wrapper around the provided provider.
  #[must_use]
  pub fn new(provider: LocalClusterProvider) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(provider) }
  }
}

impl Clone for LocalClusterProviderShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<LocalClusterProvider> for LocalClusterProviderShared {
  fn with_read<R>(&self, f: impl FnOnce(&LocalClusterProvider) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut LocalClusterProvider) -> R) -> R {
    self.inner.with_write(f)
  }
}
