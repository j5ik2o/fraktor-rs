//! Shared wrapper for LocalClusterProvider implementations.

use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeMutex,
  sync::{ArcShared, SharedAccess},
};

use super::LocalClusterProvider;

/// Shared wrapper for [`LocalClusterProvider`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying provider, allowing safe
/// concurrent access from multiple owners.
pub struct LocalClusterProviderShared {
  inner: ArcShared<RuntimeMutex<LocalClusterProvider>>,
}

impl LocalClusterProviderShared {
  /// Creates a new shared wrapper around the provided provider.
  #[must_use]
  pub fn new(provider: LocalClusterProvider) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(provider)) }
  }
}

impl Clone for LocalClusterProviderShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<LocalClusterProvider> for LocalClusterProviderShared {
  fn with_read<R>(&self, f: impl FnOnce(&LocalClusterProvider) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut LocalClusterProvider) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
