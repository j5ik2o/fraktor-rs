//! Inbound transport types for receiving frames from remote peers.

mod frame;
mod handler;
mod shared;

pub use frame::InboundFrame;
pub use handler::TransportInbound;
pub use shared::TransportInboundShared;
