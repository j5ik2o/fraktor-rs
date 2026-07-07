//! Sharding query commands and responses.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
  vec::Vec,
};
use core::time::Duration;

#[cfg(test)]
#[path = "sharding_query_test.rs"]
mod tests;

/// Query command for shard region and cluster sharding snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardingQuery {
  /// Returns the current shard region state, including shard and entity membership.
  GetShardRegionState,
  /// Returns entity counts per shard in the current region.
  GetShardRegionStats,
  /// Returns entity counts per region across the entire cluster.
  GetClusterShardingStats {
    /// Maximum time to wait for all shard regions to respond.
    timeout: Duration,
  },
  /// Returns the addresses of all registered shard regions.
  GetCurrentRegions,
}

/// Response to a sharding query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardingQueryResponse {
  /// Current shard region state.
  CurrentShardRegionState {
    /// Shards currently hosted by the region.
    shards: Vec<ShardState>,
    /// Shard identifiers that failed to respond within the query timeout.
    failed: BTreeSet<String>,
  },
  /// Entity counts for the current shard region.
  ShardRegionStats {
    /// Mapping from shard identifier to active entity count.
    stats:  BTreeMap<String, u32>,
    /// Shard identifiers that failed to respond within the query timeout.
    failed: BTreeSet<String>,
  },
  /// Entity counts for all shard regions in the cluster.
  ClusterShardingStats {
    /// Mapping from region address to region statistics.
    regions: BTreeMap<String, ShardRegionStatsSnapshot>,
  },
  /// Addresses of all registered shard regions.
  CurrentRegions {
    /// Region addresses currently registered in the cluster.
    regions: BTreeSet<String>,
  },
}

/// State of one shard within a shard region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardState {
  /// Shard identifier.
  pub shard_id:   String,
  /// Entity identifiers currently hosted by the shard.
  pub entity_ids: BTreeSet<String>,
}

/// Entity counts for one shard region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardRegionStatsSnapshot {
  /// Mapping from shard identifier to active entity count.
  pub stats:  BTreeMap<String, u32>,
  /// Shard identifiers that failed to respond within the query timeout.
  pub failed: BTreeSet<String>,
}
