//! Shared wrapper for RemoteTransport implementations.

use alloc::{boxed::Box, string::String};

use fraktor_actor_rs::core::event_stream::CorrelationId;
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{
  RemoteTransport, TransportBackpressureHookShared, TransportBind, TransportChannel, TransportEndpoint, TransportError,
  TransportHandle, TransportInboundShared,
};

/// Shared wrapper that provides thread-safe access to a [`RemoteTransport`]
/// implementation.
///
/// This adapter wraps a transport in a `ToolboxMutex`, allowing it to be shared
/// across multiple owners while satisfying the `&mut self` requirement of
/// `RemoteTransport` methods.
///
/// # Usage
///
/// 1. Create a shared wrapper: `RemoteTransportShared::new(transport)`
/// 2. Clone and share as needed
/// 3. Call transport methods through the wrapper (automatically acquires lock)
pub struct RemoteTransportShared<TB: RuntimeToolbox + 'static> {
  inner:  ArcShared<ToolboxMutex<Box<dyn RemoteTransport<TB>>, TB>>,
  scheme: String,
}

impl<TB: RuntimeToolbox + 'static> RemoteTransportShared<TB> {
  /// Creates a new shared wrapper around the provided transport implementation.
  pub fn new(transport: Box<dyn RemoteTransport<TB>>) -> Self {
    let scheme = transport.scheme().into();
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(transport)), scheme }
  }

  /// Returns a reference to the inner shared mutex.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<ToolboxMutex<Box<dyn RemoteTransport<TB>>, TB>> {
    &self.inner
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for RemoteTransportShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), scheme: self.scheme.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> RemoteTransport<TB> for RemoteTransportShared<TB> {
  fn scheme(&self) -> &str {
    &self.scheme
  }

  fn spawn_listener(&mut self, bind: &TransportBind) -> Result<TransportHandle, TransportError> {
    self.inner.lock().spawn_listener(bind)
  }

  fn open_channel(&mut self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError> {
    self.inner.lock().open_channel(endpoint)
  }

  fn send(
    &mut self,
    channel: &TransportChannel,
    payload: &[u8],
    correlation_id: CorrelationId,
  ) -> Result<(), TransportError> {
    self.inner.lock().send(channel, payload, correlation_id)
  }

  fn close(&mut self, channel: &TransportChannel) {
    self.inner.lock().close(channel)
  }

  fn install_backpressure_hook(&mut self, hook: TransportBackpressureHookShared) {
    self.inner.lock().install_backpressure_hook(hook)
  }

  fn install_inbound_handler(&mut self, handler: TransportInboundShared<TB>) {
    self.inner.lock().install_inbound_handler(handler)
  }
}
