//! Shared wrapper for `ClusterProvider` implementations.

use alloc::boxed::Box;
use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeMutex, RuntimeToolbox},
  sync::{ArcShared, SharedAccess},
};

use crate::core::cluster_provider::ClusterProvider;

/// Shared wrapper enabling interior mutability for [`ClusterProvider`].
///
/// This adapter wraps a provider in a `RuntimeMutex`, allowing callers to
/// obtain mutable access through [`SharedAccess`] without requiring a mutable
/// handle to the wrapper itself.
pub struct ClusterProviderShared<TB: RuntimeToolbox + 'static> {
  inner:   ArcShared<RuntimeMutex<Box<dyn ClusterProvider>>>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> ClusterProviderShared<TB> {
  /// Creates a new shared wrapper around the given provider.
  #[must_use]
  pub fn new(provider: Box<dyn ClusterProvider>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(provider)), _marker: PhantomData }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub fn from_inner(inner: ArcShared<RuntimeMutex<Box<dyn ClusterProvider>>>) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<Box<dyn ClusterProvider>>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox> Clone for ClusterProviderShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
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
