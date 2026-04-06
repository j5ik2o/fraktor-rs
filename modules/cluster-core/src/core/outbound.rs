//! Outbound message pipeline coordination.
//!
//! This module manages outbound message routing, action dispatching, and
//! envelope state tracking.

mod outbound_action;
mod outbound_envelope;
mod outbound_event;
mod outbound_pipeline;
mod outbound_state;

pub use outbound_action::OutboundAction;
pub use outbound_envelope::OutboundEnvelope;
pub use outbound_event::OutboundEvent;
pub use outbound_pipeline::OutboundPipeline;
pub use outbound_state::OutboundState;
