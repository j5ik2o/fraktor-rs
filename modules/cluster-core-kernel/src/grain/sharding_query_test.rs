use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
};
use core::time::Duration;

use super::{ShardRegionStatsSnapshot, ShardState, ShardingQuery, ShardingQueryResponse};

#[test]
fn query_variants_are_constructible() {
  assert_eq!(ShardingQuery::GetShardRegionState, ShardingQuery::GetShardRegionState);
  assert_eq!(ShardingQuery::GetShardRegionStats, ShardingQuery::GetShardRegionStats);
  assert_eq!(ShardingQuery::GetCurrentRegions, ShardingQuery::GetCurrentRegions);
  assert_eq!(
    ShardingQuery::GetClusterShardingStats { timeout: Duration::from_secs(3) },
    ShardingQuery::GetClusterShardingStats { timeout: Duration::from_secs(3) }
  );
}

#[test]
fn current_shard_region_state_response_preserves_shards_and_failures() {
  let response = ShardingQueryResponse::CurrentShardRegionState {
    shards: alloc::vec![ShardState {
      shard_id:   String::from("10"),
      entity_ids: BTreeSet::from([String::from("entity-1")]),
    }],
    failed: BTreeSet::from([String::from("11")]),
  };

  match response {
    | ShardingQueryResponse::CurrentShardRegionState { shards, failed } => {
      assert_eq!(shards.len(), 1);
      assert_eq!(shards[0].shard_id, "10");
      assert_eq!(failed, BTreeSet::from([String::from("11")]));
    },
    | _ => panic!("expected CurrentShardRegionState"),
  }
}

#[test]
fn shard_region_stats_response_preserves_counts() {
  let mut stats = BTreeMap::new();
  stats.insert(String::from("1"), 42);
  let response = ShardingQueryResponse::ShardRegionStats { stats: stats.clone(), failed: BTreeSet::new() };

  match response {
    | ShardingQueryResponse::ShardRegionStats { stats, failed } => {
      assert_eq!(stats.get("1"), Some(&42));
      assert!(failed.is_empty());
    },
    | _ => panic!("expected ShardRegionStats"),
  }
}

#[test]
fn cluster_sharding_stats_response_preserves_region_snapshots() {
  let mut stats = BTreeMap::new();
  stats.insert(String::from("2"), 5);
  let snapshot = ShardRegionStatsSnapshot { stats, failed: BTreeSet::new() };
  let mut regions = BTreeMap::new();
  regions.insert(String::from("cluster://node-a"), snapshot);

  let response = ShardingQueryResponse::ClusterShardingStats { regions: regions.clone() };

  match response {
    | ShardingQueryResponse::ClusterShardingStats { regions } => {
      assert_eq!(regions.get("cluster://node-a").unwrap().stats.get("2"), Some(&5));
    },
    | _ => panic!("expected ClusterShardingStats"),
  }
}

#[test]
fn current_regions_response_preserves_addresses() {
  let response = ShardingQueryResponse::CurrentRegions {
    regions: BTreeSet::from([String::from("cluster://node-a"), String::from("cluster://node-b")]),
  };

  match response {
    | ShardingQueryResponse::CurrentRegions { regions } => {
      assert_eq!(regions.len(), 2);
    },
    | _ => panic!("expected CurrentRegions"),
  }
}
