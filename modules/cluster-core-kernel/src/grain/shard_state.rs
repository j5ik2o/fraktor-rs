//! State of one shard within a shard region.

use alloc::{collections::BTreeSet, string::String};

/// State of one shard within a shard region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardState {
  /// Shard identifier.
  pub shard_id:   String,
  /// Entity identifiers currently hosted by the shard.
  pub entity_ids: BTreeSet<String>,
}
