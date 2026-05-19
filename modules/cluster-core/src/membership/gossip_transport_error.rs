//! Errors reported by gossip transport.

use alloc::string::String;

/// Gossip transport error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GossipTransportError {
  /// Failed to send a gossip message.
  SendFailed {
    /// Failure reason.
    reason: String,
  },
}
