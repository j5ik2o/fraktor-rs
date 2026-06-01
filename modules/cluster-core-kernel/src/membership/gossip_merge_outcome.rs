//! Gossip full-state merge outcome.

use alloc::vec::Vec;

use super::{GossipMergeConflict, GossipTombstone, NodeRecord};

/// Observable result of merging a full gossip state snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipMergeOutcome {
  /// Records applied to the local snapshot.
  pub applied_records:          Vec<NodeRecord>,
  /// Conflicting records resolved by deterministic precedence.
  pub conflicts:                Vec<GossipMergeConflict>,
  /// Tombstones added while merging terminal records.
  pub tombstones_added:         Vec<GossipTombstone>,
  /// Stale active records rejected by tombstones.
  pub stale_records_suppressed: Vec<NodeRecord>,
}

impl GossipMergeOutcome {
  /// Creates an empty merge outcome.
  #[must_use]
  pub const fn empty() -> Self {
    Self {
      applied_records:          Vec::new(),
      conflicts:                Vec::new(),
      tombstones_added:         Vec::new(),
      stale_records_suppressed: Vec::new(),
    }
  }
}
