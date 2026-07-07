//! Response to a sharding query.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
  vec::Vec,
};

use super::{shard_region_stats_snapshot::ShardRegionStatsSnapshot, shard_state::ShardState};

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
