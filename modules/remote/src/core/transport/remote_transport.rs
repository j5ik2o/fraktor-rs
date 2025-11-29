//! Trait implemented by all remoting transports.
use fraktor_actor_rs::core::event_stream::CorrelationId;
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{
  backpressure_hook::TransportBackpressureHookShared, transport_bind::TransportBind,
  transport_channel::TransportChannel, transport_endpoint::TransportEndpoint, transport_error::TransportError,
  transport_handle::TransportHandle, transport_inbound_handler::TransportInboundShared,
};

/// Abstraction over transport implementations used by remoting.
///
/// The trait is parameterized over a [`RuntimeToolbox`] to support both std and no_std
/// environments. The toolbox determines which mutex type is used for shared inbound
/// handler access.
///
/// Methods that mutate transport state take `&mut self`, while pure accessors
/// take `&self`. Callers requiring shared ownership should wrap implementations
/// in [`RemoteTransportShared`].
pub trait RemoteTransport<TB: RuntimeToolbox>: Send + 'static {
  /// Returns the URI scheme handled by this transport.
  fn scheme(&self) -> &str;

  /// Binds a listener to the specified authority.
  fn spawn_listener(&mut self, bind: &TransportBind) -> Result<TransportHandle, TransportError>;

  /// Opens or reuses an outbound channel targeting the specified endpoint.
  fn open_channel(&mut self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError>;

  /// Sends the provided payload over the channel, embedding the correlation id.
  fn send(
    &mut self,
    channel: &TransportChannel,
    payload: &[u8],
    correlation_id: CorrelationId,
  ) -> Result<(), TransportError>;

  /// Closes the provided channel if it exists.
  fn close(&mut self, channel: &TransportChannel);

  /// Registers a hook used to propagate backpressure notifications.
  fn install_backpressure_hook(&mut self, hook: TransportBackpressureHookShared);

  /// Registers a handler that receives inbound frames accepted by the transport.
  ///
  /// The handler is wrapped in a mutex from the toolbox's `MutexFamily`, allowing
  /// `on_frame(&mut self)` to be called safely from shared contexts.
  fn install_inbound_handler(&mut self, handler: TransportInboundShared<TB>);
}
