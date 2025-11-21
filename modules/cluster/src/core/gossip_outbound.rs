//! Outbound gossip payload addressed to a peer.

use alloc::string::String;

use crate::core::membership_delta::MembershipDelta;

/// Delta payload destined for a specific peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipOutbound {
  /// Target peer identifier (node id).
  pub target: String,
  /// Membership delta to send.
  pub delta: MembershipDelta,
}

impl GossipOutbound {
  /// Creates a new outbound gossip message.
  pub fn new(target: String, delta: MembershipDelta) -> Self {
    Self { target, delta }
  }
}
