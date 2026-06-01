//! Errors reported by gossip transport.

use alloc::string::String;

use super::GossipTransportHandoffError;

/// Gossip transport error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GossipTransportError {
  /// Failed to send a gossip message.
  SendFailed {
    /// Failure reason.
    reason: String,
  },
  /// Logical handoff validation failed.
  Handoff(GossipTransportHandoffError),
  /// Failed to receive a logical gossip payload.
  ReceiveFailed {
    /// Failure reason.
    reason: String,
  },
}

impl From<GossipTransportHandoffError> for GossipTransportError {
  fn from(value: GossipTransportHandoffError) -> Self {
    Self::Handoff(value)
  }
}
