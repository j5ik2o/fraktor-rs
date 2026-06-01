//! Full gossip state snapshot.

use super::{GossipTombstoneSet, MembershipSnapshot};

/// Full gossip merge unit containing membership, reachability, and tombstones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipStateSnapshot {
  /// Membership and reachability state observed at a version.
  pub membership: MembershipSnapshot,
  /// Tombstones for removed or dead members.
  pub tombstones: GossipTombstoneSet,
}

impl GossipStateSnapshot {
  /// Creates a full gossip state snapshot.
  #[must_use]
  pub const fn new(membership: MembershipSnapshot, tombstones: GossipTombstoneSet) -> Self {
    Self { membership, tombstones }
  }
}
