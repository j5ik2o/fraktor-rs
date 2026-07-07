//! Local handler for sharding observability queries.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::{String, ToString},
  vec::Vec,
};

use crate::{
  activation::VirtualActorRegistry,
  grain::{GrainKey, ShardRegionStatsSnapshot, ShardState, ShardingQuery, ShardingQueryResponse},
};

#[cfg(test)]
#[path = "sharding_query_handler_test.rs"]
mod tests;

/// Handles sharding queries against a local virtual actor registry snapshot.
pub struct ShardingQueryHandler<'a> {
  registry:           &'a VirtualActorRegistry,
  region_address:     String,
  registered_regions: &'a BTreeSet<String>,
}

impl<'a> ShardingQueryHandler<'a> {
  /// Creates a handler bound to the given registry and region metadata.
  #[must_use]
  pub const fn new(
    registry: &'a VirtualActorRegistry,
    region_address: String,
    registered_regions: &'a BTreeSet<String>,
  ) -> Self {
    Self { registry, region_address, registered_regions }
  }

  /// Executes a sharding query and returns the local response.
  #[must_use]
  pub fn handle(&self, query: ShardingQuery) -> ShardingQueryResponse {
    match query {
      | ShardingQuery::GetShardRegionState => self.current_shard_region_state(),
      | ShardingQuery::GetShardRegionStats => self.shard_region_stats(),
      | ShardingQuery::GetCurrentRegions => {
        ShardingQueryResponse::CurrentRegions { regions: self.registered_regions.clone() }
      },
      | ShardingQuery::GetClusterShardingStats { .. } => {
        let stats = self.shard_region_stats();
        let (local_stats, failed) = match stats {
          | ShardingQueryResponse::ShardRegionStats { stats, failed } => (stats, failed),
          | _ => (BTreeMap::new(), BTreeSet::new()),
        };
        let mut regions = BTreeMap::new();
        regions.insert(self.region_address.clone(), ShardRegionStatsSnapshot { stats: local_stats, failed });
        ShardingQueryResponse::ClusterShardingStats { regions }
      },
    }
  }

  fn current_shard_region_state(&self) -> ShardingQueryResponse {
    let shards = self.group_by_shard();
    ShardingQueryResponse::CurrentShardRegionState { shards, failed: BTreeSet::new() }
  }

  fn shard_region_stats(&self) -> ShardingQueryResponse {
    let stats =
      self.group_by_shard().into_iter().map(|shard| (shard.shard_id, shard.entity_ids.len() as u32)).collect();
    ShardingQueryResponse::ShardRegionStats { stats, failed: BTreeSet::new() }
  }

  fn group_by_shard(&self) -> Vec<ShardState> {
    let mut shards: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for key in self.registry.active_keys() {
      let shard_id = shard_id_for_key(&key);
      shards.entry(shard_id).or_default().insert(key.value().to_string());
    }
    shards.into_iter().map(|(shard_id, entity_ids)| ShardState { shard_id, entity_ids }).collect()
  }
}

fn shard_id_for_key(key: &GrainKey) -> String {
  key.value().split(':').next().unwrap_or("0").to_string()
}
