//! Shared wrapper for RemoteTransport implementations.

use alloc::{boxed::Box, string::String};

use fraktor_actor_rs::core::event::stream::CorrelationId;
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::{
  RemoteTransport, TransportBackpressureHookShared, TransportBind, TransportChannel, TransportEndpoint, TransportError,
  TransportHandle, inbound::TransportInboundShared,
};

/// Shared wrapper that provides thread-safe access to a [`RemoteTransport`]
/// implementation.
///
/// This adapter wraps a transport in a `RuntimeMutex`, allowing it to be shared
/// across multiple owners while satisfying the `&mut self` requirement of
/// `RemoteTransport` methods.
///
/// # Usage
///
/// Use [`SharedAccess`] methods (`with_read`/`with_write`) to access the
/// underlying transport.
///
/// Example: `transport_shared.with_write(|t| t.open_channel(&endpoint))?;`
pub struct RemoteTransportShared {
  inner:  ArcShared<RuntimeMutex<Box<dyn RemoteTransport>>>,
  scheme: String,
}

impl RemoteTransportShared {
  /// Creates a new shared wrapper around the provided transport implementation.
  pub fn new(transport: Box<dyn RemoteTransport>) -> Self {
    let scheme = transport.scheme().into();
    Self { inner: ArcShared::new(RuntimeMutex::new(transport)), scheme }
  }

  /// Returns the transport scheme (e.g., "tcp", "loopback").
  #[must_use]
  pub fn scheme(&self) -> &str {
    &self.scheme
  }
}

impl Clone for RemoteTransportShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), scheme: self.scheme.clone() }
  }
}

impl SharedAccess<Box<dyn RemoteTransport>> for RemoteTransportShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn RemoteTransport>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn RemoteTransport>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

impl RemoteTransport for RemoteTransportShared {
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

  fn install_inbound_handler(&mut self, handler: TransportInboundShared) {
    self.inner.lock().install_inbound_handler(handler)
  }
}
