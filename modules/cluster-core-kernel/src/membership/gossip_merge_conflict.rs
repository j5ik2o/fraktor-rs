//! Gossip full-state merge conflict.

use super::NodeRecord;

/// Records an identity conflict resolved by deterministic precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipMergeConflict {
  /// Record retained after applying precedence.
  pub retained: NodeRecord,
  /// Record ignored by precedence.
  pub ignored:  NodeRecord,
}

impl GossipMergeConflict {
  /// Creates a merge conflict outcome.
  #[must_use]
  pub const fn new(retained: NodeRecord, ignored: NodeRecord) -> Self {
    Self { retained, ignored }
  }
}
