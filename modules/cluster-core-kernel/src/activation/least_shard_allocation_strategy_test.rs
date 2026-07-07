use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
  vec::Vec,
};

use super::LeastShardAllocationStrategy;
use crate::activation::{RebalanceStrategySettings, ShardAllocationStrategy};

fn insert(region: &str, shards: &[&str], allocations: &mut BTreeMap<String, Vec<String>>) {
  allocations.insert(String::from(region), shards.iter().map(|shard| String::from(*shard)).collect());
}

#[test]
fn allocates_to_region_with_least_shards() {
  let strategy = LeastShardAllocationStrategy::with_defaults();
  let mut allocations = BTreeMap::new();
  insert("cluster://node-a", &["001"], &mut allocations);
  insert("cluster://node-b", &["002", "003"], &mut allocations);

  assert_eq!(strategy.allocate_shard("cluster://node-a", "004", &allocations), Some(String::from("cluster://node-a")));
}

#[test]
fn rebalances_from_overloaded_region() {
  let strategy = LeastShardAllocationStrategy::new(RebalanceStrategySettings::with_limits(1000, 1.0));
  let mut allocations = BTreeMap::new();
  insert("cluster://node-a", &["001", "002"], &mut allocations);
  insert("cluster://node-b", &[], &mut allocations);

  let rebalance = strategy.rebalance(&allocations, &BTreeSet::new());
  assert_eq!(rebalance, BTreeSet::from([String::from("001")]));
}

#[test]
fn does_not_rebalance_when_already_in_progress() {
  let strategy = LeastShardAllocationStrategy::with_defaults();
  let mut allocations = BTreeMap::new();
  insert("cluster://node-a", &["001", "002"], &mut allocations);

  let in_progress = BTreeSet::from([String::from("001")]);
  assert!(strategy.rebalance(&allocations, &in_progress).is_empty());
}

#[test]
fn does_not_rebalance_balanced_allocations() {
  let strategy = LeastShardAllocationStrategy::new(RebalanceStrategySettings::with_limits(1000, 1.0));
  let mut allocations = BTreeMap::new();
  insert("cluster://node-a", &["001"], &mut allocations);
  insert("cluster://node-b", &["002"], &mut allocations);

  assert!(strategy.rebalance(&allocations, &BTreeSet::new()).is_empty());
}
