//! Trait implemented by all remoting transports.
use fraktor_actor_rs::core::event_stream::CorrelationId;
use fraktor_utils_rs::core::sync::ArcShared;

use super::{
  backpressure_hook::TransportBackpressureHookShared, transport_bind::TransportBind,
  transport_channel::TransportChannel, transport_endpoint::TransportEndpoint, transport_error::TransportError,
  transport_handle::TransportHandle, transport_inbound_handler::TransportInbound,
};

/// Abstraction over transport implementations used by remoting.
pub trait RemoteTransport: Send + Sync + 'static {
  /// Returns the URI scheme handled by this transport.
  fn scheme(&self) -> &str;

  /// Binds a listener to the specified authority.
  fn spawn_listener(&self, bind: &TransportBind) -> Result<TransportHandle, TransportError>;

  /// Opens or reuses an outbound channel targeting the specified endpoint.
  fn open_channel(&self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError>;

  /// Sends the provided payload over the channel, embedding the correlation id.
  fn send(
    &self,
    channel: &TransportChannel,
    payload: &[u8],
    correlation_id: CorrelationId,
  ) -> Result<(), TransportError>;

  /// Closes the provided channel if it exists.
  fn close(&self, channel: &TransportChannel);

  /// Registers a hook used to propagate backpressure notifications.
  fn install_backpressure_hook(&self, hook: TransportBackpressureHookShared);

  /// Registers a handler that receives inbound frames accepted by the transport.
  fn install_inbound_handler(&self, handler: ArcShared<dyn TransportInbound>);
}
