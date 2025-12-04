//! Shared wrapper for LocalClusterProvider implementations.

use fraktor_utils_rs::{
  core::{
    runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
    sync::{ArcShared, SharedAccess},
  },
  std::runtime_toolbox::StdToolbox,
};

use crate::core::LocalClusterProvider;

/// Shared wrapper for [`LocalClusterProvider`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying provider, allowing safe
/// concurrent access from multiple owners.
pub struct SharedLocalClusterProvider {
  inner: ArcShared<ToolboxMutex<LocalClusterProvider<StdToolbox>, StdToolbox>>,
}

impl SharedLocalClusterProvider {
  /// Creates a new shared wrapper around the provided provider.
  #[must_use]
  pub fn new(provider: LocalClusterProvider<StdToolbox>) -> Self {
    Self { inner: ArcShared::new(<StdToolbox as RuntimeToolbox>::MutexFamily::create(provider)) }
  }
}

impl Clone for SharedLocalClusterProvider {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<LocalClusterProvider<StdToolbox>> for SharedLocalClusterProvider {
  fn with_read<R>(&self, f: impl FnOnce(&LocalClusterProvider<StdToolbox>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut LocalClusterProvider<StdToolbox>) -> R) -> R {
    self.inner.with_write(f)
  }
}
