//! Tokio transport bridge that connects EndpointWriter/Reader with transports.

#[cfg(test)]
mod tests;

mod bridge;
mod config;
mod handle;

pub(crate) use bridge::EndpointTransportBridge;
pub use config::EndpointTransportBridgeConfig;
pub use handle::EndpointTransportBridgeHandle;
