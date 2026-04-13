//! Shared wrapper for delivery endpoints.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, DefaultMutex};

use super::DeliveryEndpoint;

/// Shared wrapper enabling interior mutability for delivery endpoints.
pub struct DeliveryEndpointShared {
  /// Shared endpoint implementation.
  pub(crate) inner: SharedLock<Box<dyn DeliveryEndpoint>>,
}

impl DeliveryEndpointShared {
  /// Creates a new shared wrapper around the given endpoint.
  #[must_use]
  pub fn new(endpoint: Box<dyn DeliveryEndpoint>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(endpoint) }
  }
}

impl Clone for DeliveryEndpointShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn DeliveryEndpoint>> for DeliveryEndpointShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn DeliveryEndpoint>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn DeliveryEndpoint>) -> R) -> R {
    self.inner.with_write(f)
  }
}
