//! Transport abstractions for remoting.

mod factory;
mod loopback;
#[cfg(test)]
mod tests;
mod transport_bind;
mod transport_channel;
mod transport_endpoint;
mod transport_error;
mod transport_handle;

use alloc::sync::Arc;

#[allow(unused_imports)]
pub use factory::*;
use fraktor_actor_rs::core::event_stream::BackpressureSignal;
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;
pub use loopback::LoopbackTransport;
pub use transport_bind::TransportBind;
pub use transport_channel::TransportChannel;
pub use transport_endpoint::TransportEndpoint;
pub use transport_error::TransportError;
pub use transport_handle::TransportHandle;

/// Hook invoked when transport backpressure changes.
pub type BackpressureHook = Arc<dyn Fn(BackpressureSignal, &str) + Send + Sync + 'static>;

/// Transport abstraction that handles wire-level operations.
pub trait RemoteTransport<TB: RuntimeToolbox + 'static>: Send + Sync {
  /// Returns the canonical scheme identifier for this transport.
  fn scheme(&self) -> &str;
  /// Installs a backpressure hook invoked when throttling changes.
  fn install_backpressure_hook(&self, hook: BackpressureHook);
  /// Spawns a listener bound to the provided endpoint.
  fn spawn_listener(&self, bind: &TransportBind) -> Result<TransportHandle, TransportError>;
  /// Opens a channel to the remote endpoint.
  fn open_channel(&self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError>;
  /// Sends a payload through the channel.
  fn send(&self, channel: &TransportChannel, payload: &[u8]) -> Result<(), TransportError>;
  /// Closes the provided channel.
  fn close(&self, channel: TransportChannel);
}
