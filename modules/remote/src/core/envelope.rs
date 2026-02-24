//! Message envelope types for remoting communication.

mod acked_delivery;
mod deferred;
mod inbound;
mod outbound_message;
mod priority;
mod remoting;
mod system_message_envelope;

pub use acked_delivery::{ACKED_DELIVERY_ACK_FRAME_KIND, ACKED_DELIVERY_NACK_FRAME_KIND, AckedDelivery};
pub use deferred::DeferredEnvelope;
pub use inbound::InboundEnvelope;
pub use outbound_message::OutboundMessage;
pub use priority::OutboundPriority;
pub use remoting::RemotingEnvelope;
pub use system_message_envelope::{SYSTEM_MESSAGE_FRAME_KIND, SystemMessageEnvelope};
