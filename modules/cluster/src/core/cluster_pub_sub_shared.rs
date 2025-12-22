//! Shared wrapper for `ClusterPubSub` implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::ClusterPubSub;

/// Shared wrapper enabling interior mutability for [`ClusterPubSub`].
///
/// This adapter wraps a pub/sub implementation in a `ToolboxMutex`, allowing
/// callers to access mutable methods via [`SharedAccess`] without requiring a
/// mutable handle to the wrapper itself.
pub struct ClusterPubSubShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn ClusterPubSub<TB>>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> ClusterPubSubShared<TB> {
  /// Creates a new shared wrapper around the given pub/sub implementation.
  #[must_use]
  pub fn new(pub_sub: Box<dyn ClusterPubSub<TB>>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(pub_sub)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<ToolboxMutex<Box<dyn ClusterPubSub<TB>>, TB>>) -> Self {
    Self { inner }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<ToolboxMutex<Box<dyn ClusterPubSub<TB>>, TB>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox> Clone for ClusterPubSubShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn ClusterPubSub<TB>>> for ClusterPubSubShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ClusterPubSub<TB>>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ClusterPubSub<TB>>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
