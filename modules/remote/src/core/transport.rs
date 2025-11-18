//! Transport abstractions bridging remoting and physical channels.

pub mod backpressure_hook;
pub mod factory;
pub mod loopback_transport;
pub mod remote_transport;
pub mod transport_bind;
pub mod transport_channel;
pub mod transport_endpoint;
pub mod transport_error;
pub mod transport_handle;

#[cfg(test)]
mod tests;

pub use backpressure_hook::TransportBackpressureHook;
pub use factory::TransportFactory;
pub use loopback_transport::LoopbackTransport;
pub use remote_transport::RemoteTransport;
pub use transport_bind::TransportBind;
pub use transport_channel::TransportChannel;
pub use transport_endpoint::TransportEndpoint;
pub use transport_error::TransportError;
pub use transport_handle::TransportHandle;
