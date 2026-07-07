//! Entity counts for one shard region.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
};

/// Entity counts for one shard region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardRegionStatsSnapshot {
  /// Mapping from shard identifier to active entity count.
  pub stats:  BTreeMap<String, u32>,
  /// Shard identifiers that failed to respond within the query timeout.
  pub failed: BTreeSet<String>,
}
