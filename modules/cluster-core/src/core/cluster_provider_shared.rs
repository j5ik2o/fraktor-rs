//! Shared wrapper for `ClusterProvider` implementations.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::core::cluster_provider::ClusterProvider;

/// Shared wrapper enabling interior mutability for [`ClusterProvider`].
///
/// This adapter wraps a provider in a `SharedLock`, allowing callers to
/// obtain mutable access through [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
///
/// ```compile_fail
/// use fraktor_cluster_core_rs::core::cluster_provider_shared::ClusterProviderShared;
///
/// let shared: ClusterProviderShared = todo!();
/// let _ = shared.inner();
/// ```
pub struct ClusterProviderShared {
  inner: SharedLock<Box<dyn ClusterProvider>>,
}

impl ClusterProviderShared {
  /// Creates a new shared wrapper around the given provider.
  #[must_use]
  pub fn new(provider: Box<dyn ClusterProvider>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(provider) }
  }
}

impl Clone for ClusterProviderShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn ClusterProvider>> for ClusterProviderShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ClusterProvider>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ClusterProvider>) -> R) -> R {
    self.inner.with_write(f)
  }
}
