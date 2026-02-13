//! Shared wrapper for `ClusterProvider` implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::cluster_provider::ClusterProvider;

/// Shared wrapper enabling interior mutability for [`ClusterProvider`].
///
/// This adapter wraps a provider in a `ToolboxMutex`, allowing callers to
/// obtain mutable access through [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct ClusterProviderShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn ClusterProvider>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> ClusterProviderShared<TB> {
  /// Creates a new shared wrapper around the given provider.
  #[must_use]
  pub fn new(provider: Box<dyn ClusterProvider>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(provider)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<ToolboxMutex<Box<dyn ClusterProvider>, TB>>) -> Self {
    Self { inner }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<ToolboxMutex<Box<dyn ClusterProvider>, TB>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox> Clone for ClusterProviderShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn ClusterProvider>> for ClusterProviderShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ClusterProvider>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ClusterProvider>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
