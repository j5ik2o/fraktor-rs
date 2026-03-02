//! Shared wrapper for TransportInbound implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::handler::TransportInbound;

/// Shared wrapper for [`TransportInbound`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying handler, allowing safe
/// concurrent access from multiple owners.
pub struct TransportInboundShared {
  inner: ArcShared<RuntimeMutex<Box<dyn TransportInbound + 'static>>>,
}

impl TransportInboundShared {
  /// Creates a new shared wrapper around the provided handler.
  #[must_use]
  pub fn new(handler: Box<dyn TransportInbound + 'static>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(handler)) }
  }
}

impl Clone for TransportInboundShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn TransportInbound + 'static>> for TransportInboundShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn TransportInbound + 'static>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn TransportInbound + 'static>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
