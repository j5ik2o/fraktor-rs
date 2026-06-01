//! Gossip transport abstraction.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};

use super::{GossipEnvelope, GossipOutbound, GossipTransportError, MembershipDelta};

/// Transport used to exchange gossip deltas.
pub trait GossipTransport {
  /// Sends a gossip outbound payload.
  ///
  /// # Errors
  ///
  /// Returns an error if transport failed to send.
  fn send(&mut self, outbound: GossipOutbound) -> Result<(), GossipTransportError>;

  /// Polls incoming deltas from peers.
  fn poll_deltas(&mut self) -> Vec<(String, MembershipDelta)>;

  /// Sends an identity-aware logical gossip envelope.
  ///
  /// # Errors
  ///
  /// Returns an error if transport failed to validate or send the envelope.
  fn send_envelope(&mut self, _envelope: GossipEnvelope, _now_tick: u64) -> Result<(), GossipTransportError> {
    Err(GossipTransportError::SendFailed { reason: "envelope handoff is unsupported".to_string() })
  }

  /// Polls incoming identity-aware logical gossip envelopes.
  fn poll_envelopes(&mut self) -> Vec<Result<GossipEnvelope, GossipTransportError>> {
    Vec::new()
  }
}
