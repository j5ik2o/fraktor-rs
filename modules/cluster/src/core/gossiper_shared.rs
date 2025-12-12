//! Shared wrapper for `Gossiper` implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::Gossiper;

/// Shared wrapper enabling interior mutability for [`Gossiper`].
///
/// This adapter wraps a gossiper in a `ToolboxMutex`, allowing callers to
/// access mutable methods via [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct GossiperShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn Gossiper>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> GossiperShared<TB> {
  /// Creates a new shared wrapper around the given gossiper.
  #[must_use]
  pub fn new(gossiper: Box<dyn Gossiper>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(gossiper)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<ToolboxMutex<Box<dyn Gossiper>, TB>>) -> Self {
    Self { inner }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<ToolboxMutex<Box<dyn Gossiper>, TB>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox> Clone for GossiperShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn Gossiper>> for GossiperShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Gossiper>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Gossiper>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
