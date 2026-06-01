//! Full gossip state snapshot.

use super::{GossipSeenDigest, GossipTombstoneSet, MembershipSnapshot};

/// Full gossip merge unit containing membership, reachability, and tombstones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipStateSnapshot {
  /// Membership and reachability state observed at a version.
  pub membership:  MembershipSnapshot,
  /// Tombstones for removed or dead members.
  pub tombstones:  GossipTombstoneSet,
  /// Seen versions reported by peer identities.
  pub seen_digest: GossipSeenDigest,
}

impl GossipStateSnapshot {
  /// Creates a full gossip state snapshot.
  #[must_use]
  pub const fn new(membership: MembershipSnapshot, tombstones: GossipTombstoneSet) -> Self {
    Self { membership, tombstones, seen_digest: GossipSeenDigest::new() }
  }

  /// Creates a full gossip state snapshot with an explicit seen digest.
  #[must_use]
  pub const fn new_with_seen_digest(
    membership: MembershipSnapshot,
    tombstones: GossipTombstoneSet,
    seen_digest: GossipSeenDigest,
  ) -> Self {
    Self { membership, tombstones, seen_digest }
  }
}
