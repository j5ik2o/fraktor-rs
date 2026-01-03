//! Message envelope types for remoting communication.

mod deferred;
mod inbound;
mod outbound_message;
mod priority;
mod remoting;

pub use deferred::DeferredEnvelope;
pub use inbound::InboundEnvelope;
pub use outbound_message::OutboundMessage;
pub use priority::OutboundPriority;
pub use remoting::RemotingEnvelope;
