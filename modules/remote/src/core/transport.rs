//! Transport abstractions bridging remoting and physical channels.

#[cfg(test)]
mod tests;

mod backpressure_hook;
mod factory;
mod loopback_transport;
mod remote_transport;
mod remote_transport_shared;
mod tokio_transport_config;
mod transport_bind;
mod transport_channel;
mod transport_endpoint;
mod transport_error;
mod transport_handle;
mod transport_inbound_frame;
mod transport_inbound_handler;

pub use backpressure_hook::{TransportBackpressureHook, TransportBackpressureHookShared};
pub use factory::TransportFactory;
pub use loopback_transport::LoopbackTransport;
pub use remote_transport::RemoteTransport;
pub use remote_transport_shared::RemoteTransportShared;
pub use tokio_transport_config::TokioTransportConfig;
pub use transport_bind::TransportBind;
pub use transport_channel::TransportChannel;
pub use transport_endpoint::TransportEndpoint;
pub use transport_error::TransportError;
pub use transport_handle::TransportHandle;
pub use transport_inbound_frame::InboundFrame;
pub use transport_inbound_handler::TransportInbound;
