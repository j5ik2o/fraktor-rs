//! Sharding query commands and responses.

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
