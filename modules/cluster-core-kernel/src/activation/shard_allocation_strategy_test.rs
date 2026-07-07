use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
  vec::Vec,
};

use super::{ExternalShardLocations, ShardAllocationStrategy};

struct ExternalAllocationStrategy {
  locations: ExternalShardLocations,
}

impl ExternalAllocationStrategy {
  const fn new(locations: ExternalShardLocations) -> Self {
    Self { locations }
  }
}

impl ShardAllocationStrategy for ExternalAllocationStrategy {
  fn allocate_shard(
    &self,
    requester: &str,
    shard_id: &str,
    current_allocations: &BTreeMap<String, Vec<String>>,
  ) -> Option<String> {
    if let Some(region) = self.locations.region_for_shard(shard_id) {
      if current_allocations.contains_key(region) {
        return Some(String::from(region));
      }
    }

    if current_allocations.contains_key(requester) {
      return Some(String::from(requester));
    }

    current_allocations.keys().next().cloned()
  }

  fn rebalance(
    &self,
    _current_allocations: &BTreeMap<String, Vec<String>>,
    _rebalance_in_progress: &BTreeSet<String>,
  ) -> BTreeSet<String> {
    BTreeSet::new()
  }
}

#[test]
fn external_locations_store_and_lookup_shard_mappings() {
  let mut locations = ExternalShardLocations::new();
  locations.insert("10", "cluster://node-a");
  locations.insert("11", "cluster://node-b");

  assert_eq!(locations.region_for_shard("10"), Some("cluster://node-a"));
  assert_eq!(locations.region_for_shard("99"), None);
  assert_eq!(locations.locations().len(), 2);
}

#[test]
fn external_strategy_uses_configured_location_when_region_is_available() {
  let mut locations = ExternalShardLocations::new();
  locations.insert("10", "cluster://node-b");
  let strategy = ExternalAllocationStrategy::new(locations);

  let mut current = BTreeMap::new();
  current.insert(String::from("cluster://node-a"), Vec::new());
  current.insert(String::from("cluster://node-b"), Vec::new());

  assert_eq!(strategy.allocate_shard("cluster://node-a", "10", &current), Some(String::from("cluster://node-b")));
}

#[test]
fn external_strategy_falls_back_to_requester() {
  let strategy = ExternalAllocationStrategy::new(ExternalShardLocations::new());

  let mut current = BTreeMap::new();
  current.insert(String::from("cluster://node-a"), Vec::new());

  assert_eq!(strategy.allocate_shard("cluster://node-a", "10", &current), Some(String::from("cluster://node-a")));
}

#[test]
fn external_strategy_rebalance_returns_empty_set() {
  let strategy = ExternalAllocationStrategy::new(ExternalShardLocations::new());
  assert!(strategy.rebalance(&BTreeMap::new(), &BTreeSet::new()).is_empty());
}
