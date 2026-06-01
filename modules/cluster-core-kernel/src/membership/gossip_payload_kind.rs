//! Logical gossip payload category.

/// Distinguishes protocol payloads carried by a gossip envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GossipPayloadKind {
  /// Membership delta diffusion payload.
  Delta,
  /// Full membership and reachability state payload.
  FullState,
  /// Peer seen-version digest payload.
  SeenDigest,
  /// Dedicated cluster heartbeat request payload.
  HeartbeatRequest,
  /// Dedicated cluster heartbeat response payload.
  HeartbeatResponse,
  /// Cross data center heartbeat payload.
  CrossDcHeartbeat,
}
