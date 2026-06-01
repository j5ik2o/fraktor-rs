//! Gossip tombstone prune outcome.

use alloc::vec::Vec;

use super::GossipTombstone;

/// Observable result of pruning retained tombstones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipTombstonePruneOutcome {
  /// Tombstones removed by the retention rule.
  pub pruned: Vec<GossipTombstone>,
}

impl GossipTombstonePruneOutcome {
  /// Creates a prune outcome from removed tombstones.
  #[must_use]
  pub const fn new(pruned: Vec<GossipTombstone>) -> Self {
    Self { pruned }
  }
}
