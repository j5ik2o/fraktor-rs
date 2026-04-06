//! In-memory envelope types for messages crossing the remote boundary.
//!
//! These are the pure data representations handed between the higher-level API
//! and the wire layer. They are intentionally independent of the `wire` module's
//! PDU types so that the core can evolve the two layers separately.

#[cfg(test)]
mod tests;

mod inbound_envelope;
mod outbound_envelope;
mod priority;

pub use inbound_envelope::InboundEnvelope;
pub use outbound_envelope::OutboundEnvelope;
pub use priority::OutboundPriority;
