//! Shared wrapper for TransportInbound implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::transport_inbound_handler::TransportInbound;

/// Shared wrapper for [`TransportInbound`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying handler, allowing safe
/// concurrent access from multiple owners.
pub struct TransportInboundShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn TransportInbound + 'static>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> TransportInboundShared<TB> {
  /// Creates a new shared wrapper around the provided handler.
  #[must_use]
  pub fn new(handler: Box<dyn TransportInbound + 'static>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(handler)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for TransportInboundShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn TransportInbound + 'static>> for TransportInboundShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn TransportInbound + 'static>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn TransportInbound + 'static>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
