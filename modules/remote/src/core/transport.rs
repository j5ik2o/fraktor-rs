//! Transport abstractions bridging remoting and physical channels.

#[cfg(test)]
mod tests;

pub mod backpressure_hook;
mod factory;
mod loopback_transport;
mod remote_transport;
mod transport_bind;
mod transport_channel;
mod transport_endpoint;
mod transport_error;
mod transport_handle;

pub use backpressure_hook::TransportBackpressureHook;
pub use factory::TransportFactory;
pub use loopback_transport::LoopbackTransport;
pub use remote_transport::RemoteTransport;
pub use transport_bind::TransportBind;
pub use transport_channel::TransportChannel;
pub use transport_endpoint::TransportEndpoint;
pub use transport_error::TransportError;
pub use transport_handle::TransportHandle;
