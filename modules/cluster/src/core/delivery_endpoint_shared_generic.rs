//! Shared wrapper for delivery endpoints.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::DeliveryEndpoint;

/// Shared wrapper enabling interior mutability for delivery endpoints.
pub struct DeliveryEndpointSharedGeneric<TB: RuntimeToolbox + 'static> {
  /// Shared endpoint implementation.
  pub inner: ArcShared<ToolboxMutex<Box<dyn DeliveryEndpoint<TB>>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> DeliveryEndpointSharedGeneric<TB> {
  /// Creates a new shared wrapper around the given endpoint.
  #[must_use]
  pub fn new(endpoint: Box<dyn DeliveryEndpoint<TB>>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(endpoint)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub const fn from_inner(inner: ArcShared<ToolboxMutex<Box<dyn DeliveryEndpoint<TB>>, TB>>) -> Self {
    Self { inner }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<ToolboxMutex<Box<dyn DeliveryEndpoint<TB>>, TB>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for DeliveryEndpointSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn DeliveryEndpoint<TB>>> for DeliveryEndpointSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn DeliveryEndpoint<TB>>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn DeliveryEndpoint<TB>>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
