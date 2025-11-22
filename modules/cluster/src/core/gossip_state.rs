//! Gossip state machine phases.

/// Represents the convergence phase of gossip dissemination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GossipState {
  /// Currently diffusing a new delta to peers.
  Diffusing,
  /// Reconciling missing ranges or conflicts.
  Reconciling,
  /// All known peers have confirmed the latest version.
  Confirmed,
}
