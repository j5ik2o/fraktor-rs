//! Shared wrapper for `ClusterProvider` implementations.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use crate::core::cluster_provider::ClusterProvider;

/// Shared wrapper enabling interior mutability for [`ClusterProvider`].
///
/// This adapter wraps a provider in a `RuntimeMutex`, allowing callers to
/// obtain mutable access through [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct ClusterProviderShared {
  inner: ArcShared<RuntimeMutex<Box<dyn ClusterProvider>>>,
}

impl ClusterProviderShared {
  /// Creates a new shared wrapper around the given provider.
  #[must_use]
  pub fn new(provider: Box<dyn ClusterProvider>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(provider)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<RuntimeMutex<Box<dyn ClusterProvider>>>) -> Self {
    Self { inner }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<Box<dyn ClusterProvider>>> {
    self.inner.clone()
  }
}

impl Clone for ClusterProviderShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn ClusterProvider>> for ClusterProviderShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ClusterProvider>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ClusterProvider>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
