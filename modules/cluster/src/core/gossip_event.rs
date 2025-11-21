//! Events emitted by gossip convergence.

use alloc::string::String;

use crate::core::membership_version::MembershipVersion;

/// Gossip lifecycle events for observability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GossipEvent {
  /// Delta was diffused to peers.
  Disseminated {
    /// Target peers included in this diffusion round.
    peers:   usize,
    /// Version diffused.
    version: MembershipVersion,
  },
  /// Missing range was requested, entering reconciling.
  ReconcilingRequested {
    /// Peer that reported missing range.
    peer:          String,
    /// Current local version.
    local_version: MembershipVersion,
  },
  /// Conflicting view detected.
  ConflictDetected {
    /// Peer that sent the conflicting delta.
    peer:           String,
    /// Local version at detection.
    local_version:  MembershipVersion,
    /// Remote version observed.
    remote_version: MembershipVersion,
  },
  /// All peers have confirmed the latest version.
  Confirmed {
    /// Version that reached confirmation.
    version: MembershipVersion,
  },
}
