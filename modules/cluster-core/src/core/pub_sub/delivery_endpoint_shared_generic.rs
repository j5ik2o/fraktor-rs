//! Shared wrapper for delivery endpoints.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::DeliveryEndpoint;

/// Shared wrapper enabling interior mutability for delivery endpoints.
pub struct DeliveryEndpointShared {
  /// Shared endpoint implementation.
  pub(crate) inner: ArcShared<RuntimeMutex<Box<dyn DeliveryEndpoint>>>,
}

impl DeliveryEndpointShared {
  /// Creates a new shared wrapper around the given endpoint.
  #[must_use]
  pub fn new(endpoint: Box<dyn DeliveryEndpoint>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(endpoint)) }
  }

  /// Creates a wrapper from an existing shared mutex.
  #[must_use]
  pub const fn from_inner(inner: ArcShared<RuntimeMutex<Box<dyn DeliveryEndpoint>>>) -> Self {
    Self { inner }
  }
}

impl Clone for DeliveryEndpointShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn DeliveryEndpoint>> for DeliveryEndpointShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn DeliveryEndpoint>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn DeliveryEndpoint>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
