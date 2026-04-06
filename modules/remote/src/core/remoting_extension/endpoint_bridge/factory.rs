//! Factory trait for spawning the endpoint transport bridge.

use alloc::boxed::Box;

use super::{config::EndpointBridgeConfig, handle::EndpointBridgeHandle};
use crate::core::remoting_extension::error::RemotingError;

/// Factory that spawns an [`EndpointBridgeHandle`] from an
/// [`EndpointBridgeConfig`].
///
/// Implemented by adapter crates (e.g. `EndpointTransportBridgeFactory` in
/// the std adapter) and registered with
/// [`RemotingControlHandle`](super::super::RemotingControlHandle) before the
/// runtime is bootstrapped.
pub trait EndpointBridgeFactory: Send + Sync {
  /// Spawns the bridge and returns a handle that keeps it alive.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::TransportUnavailable`] when the spawned bridge
  /// cannot bind its listener or wire up the transport.
  fn spawn(&self, config: EndpointBridgeConfig) -> Result<Box<dyn EndpointBridgeHandle>, RemotingError>;
}
