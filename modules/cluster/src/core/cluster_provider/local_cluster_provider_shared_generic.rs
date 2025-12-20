//! Shared wrapper for LocalClusterProvider implementations.

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::LocalClusterProvider;

/// Shared wrapper for [`LocalClusterProvider`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying provider, allowing safe
/// concurrent access from multiple owners.
pub struct LocalClusterProviderSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<LocalClusterProvider<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> LocalClusterProviderSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided provider.
  #[must_use]
  pub fn new(provider: LocalClusterProvider<TB>) -> Self {
    Self { inner: ArcShared::new(<TB as RuntimeToolbox>::MutexFamily::create(provider)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for LocalClusterProviderSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<LocalClusterProvider<TB>> for LocalClusterProviderSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&LocalClusterProvider<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut LocalClusterProvider<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
