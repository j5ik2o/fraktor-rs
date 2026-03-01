//! Shared wrapper for `Gossiper` implementations.

use alloc::boxed::Box;
use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeMutex, RuntimeToolbox},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::Gossiper;

/// Shared wrapper enabling interior mutability for [`Gossiper`].
///
/// This adapter wraps a gossiper in a `RuntimeMutex`, allowing callers to
/// access mutable methods via [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct GossiperShared<TB: RuntimeToolbox + 'static> {
  inner:   ArcShared<RuntimeMutex<Box<dyn Gossiper>>>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> GossiperShared<TB> {
  /// Creates a new shared wrapper around the given gossiper.
  #[must_use]
  pub fn new(gossiper: Box<dyn Gossiper>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(gossiper)), _marker: PhantomData }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<RuntimeMutex<Box<dyn Gossiper>>>) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<Box<dyn Gossiper>>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox> Clone for GossiperShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
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
