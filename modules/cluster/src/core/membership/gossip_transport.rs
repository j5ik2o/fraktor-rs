//! Gossip transport abstraction.

use alloc::{string::String, vec::Vec};

use super::{GossipOutbound, GossipTransportError, MembershipDelta};

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
}
