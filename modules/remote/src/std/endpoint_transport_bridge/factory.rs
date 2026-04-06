//! Factory that spawns [`EndpointTransportBridge`] via the
//! [`EndpointBridgeFactory`] port defined in `core`.

use alloc::{boxed::Box, format};

use crate::{
  core::remoting_extension::{
    RemotingError,
    endpoint_bridge::{EndpointBridgeConfig, EndpointBridgeFactory, EndpointBridgeHandle},
  },
  std::endpoint_transport_bridge::EndpointTransportBridge,
};

/// Default tokio-backed bridge factory used by the std remoting extension.
pub struct EndpointTransportBridgeFactory;

impl EndpointTransportBridgeFactory {
  /// Creates a new factory instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for EndpointTransportBridgeFactory {
  fn default() -> Self {
    Self::new()
  }
}

impl EndpointBridgeFactory for EndpointTransportBridgeFactory {
  fn spawn(&self, config: EndpointBridgeConfig) -> Result<Box<dyn EndpointBridgeHandle>, RemotingError> {
    let handle = EndpointTransportBridge::spawn(config)
      .map_err(|error| RemotingError::TransportUnavailable(format!("{error:?}")))?;
    Ok(Box::new(handle))
  }
}
