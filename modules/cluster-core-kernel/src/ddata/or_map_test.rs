use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use crate::ddata::{DeltaReplicatedData, GCounter, ORMap, RemovedNodePruning, ReplicatedData, SelfUniqueAddress};

fn unique_address(index: usize) -> UniqueAddress {
  match index % 4 {
    | 0 => UniqueAddress::new(Address::new("sys", "node-a", 2552), 1),
    | 1 => UniqueAddress::new(Address::new("sys", "node-b", 2553), 2),
    | 2 => UniqueAddress::new(Address::new("sys", "node-c", 2554), 3),
    | _ => UniqueAddress::new(Address::new("sys", "node-d", 2555), 4),
  }
}

fn self_address(index: usize) -> SelfUniqueAddress {
  SelfUniqueAddress::new(unique_address(index))
}

fn counter(node: &SelfUniqueAddress, n: u64) -> GCounter {
  GCounter::new().increment(node, n).expect("counter fits")
}

fn build_replica(node_index: usize, ops: &[(u8, u8)]) -> ORMap<u8, GCounter> {
  let node = self_address(node_index);
  let mut map = ORMap::new();
  for (kind, key) in ops {
    map = match kind {
      | 0 => map.put(&node, *key, counter(&node, 1)),
      | 1 => map.update(&node, *key, GCounter::new(), |value| value.increment(&node, 1).expect("counter fits")),
      | _ => map.remove(key),
    };
  }
  map
}

fn op_strategy() -> impl Strategy<Value = Vec<(u8, u8)>> {
  prop::collection::vec((0_u8..3, 0_u8..4), 0..15)
}

#[test]
fn put_then_get_returns_value() {
  let node = self_address(0);
  let map = ORMap::new().put(&node, 1_u8, counter(&node, 5));

  assert_eq!(map.get(&1).expect("value present").value().expect("counter fits"), 5);
  assert!(map.contains_key(&1));
  assert_eq!(map.len(), 1);
}

#[test]
fn update_applies_modify_to_existing_or_initial() {
  let node = self_address(0);
  let map = ORMap::new()
    .update(&node, 1_u8, GCounter::new(), |value| value.increment(&node, 2).expect("counter fits"))
    .update(&node, 1_u8, GCounter::new(), |value| value.increment(&node, 3).expect("counter fits"));

  assert_eq!(map.get(&1).expect("value present").value().expect("counter fits"), 5);
}

#[test]
fn remove_hides_entry() {
  let node = self_address(0);
  let map = ORMap::new().put(&node, 1_u8, counter(&node, 1)).remove(&1);

  assert!(!map.contains_key(&1));
  assert!(map.is_empty());
}

#[test]
fn concurrent_value_updates_merge() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let left = ORMap::new().update(&node_a, 1_u8, GCounter::new(), |value| value.increment(&node_a, 5).expect("fits"));
  let right = ORMap::new().update(&node_b, 1_u8, GCounter::new(), |value| value.increment(&node_b, 3).expect("fits"));

  let merged = left.merge(&right);

  assert_eq!(merged.get(&1).expect("value present").value().expect("counter fits"), 8);
}

#[test]
fn remove_keeps_key_under_concurrent_update() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let shared = ORMap::new().update(&node_a, 1_u8, GCounter::new(), |value| value.increment(&node_a, 1).expect("fits"));

  let removed = shared.reset_delta().remove(&1);
  let updated =
    shared.reset_delta().update(&node_b, 1_u8, GCounter::new(), |value| value.increment(&node_b, 1).expect("fits"));

  assert!(removed.merge(&updated).contains_key(&1));
  assert!(updated.merge(&removed).contains_key(&1));
}

#[test]
fn merge_does_not_reintroduce_old_value_after_remove_and_readd() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let old = ORMap::new().put(&node_a, 1_u8, counter(&node_b, 3));
  let readded = old.remove(&1).put(&node_a, 1_u8, counter(&node_a, 5));

  let left = readded.merge(&old);
  let right = old.merge(&readded);

  assert_eq!(left.get(&1).expect("value present").value().expect("counter fits"), 5);
  assert_eq!(right.get(&1).expect("value present").value().expect("counter fits"), 5);
}

#[test]
fn merge_is_order_independent() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let left = ORMap::new().put(&node_a, 1_u8, counter(&node_a, 2));
  let right = ORMap::new().put(&node_b, 2_u8, counter(&node_b, 4));

  assert_eq!(left.merge(&right), right.merge(&left));
}

#[test]
fn delta_application_matches_full_merge() {
  let node = self_address(0);
  let added = ORMap::new().put(&node, 1_u8, counter(&node, 1));
  let first_delta = added.delta().expect("local change produces a delta");

  let removed = added.reset_delta().remove(&1);
  let second_delta = removed.delta().expect("local change produces a delta");

  let replicated = ORMap::new().merge_delta(&first_delta).merge_delta(&second_delta);

  assert_eq!(replicated, removed);
  assert!(!replicated.contains_key(&1));
}

#[test]
fn pruning_collapses_value_and_key_contributions() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let removed = unique_address(0);
  let collapse = unique_address(1);
  let map = ORMap::new().put(&node_a, 1_u8, counter(&node_a, 5)).put(&node_b, 2_u8, counter(&node_b, 3));

  assert!(map.need_pruning_from(&removed));

  let pruned = map.prune(&removed, &collapse).expect("counter collapse fits");

  assert!(!pruned.need_pruning_from(&removed));
  assert_eq!(pruned.get(&1).expect("value present").value().expect("counter fits"), 5);
  assert_eq!(pruned.get(&2).expect("value present").value().expect("counter fits"), 3);
}

proptest! {
  #[test]
  fn merge_is_commutative(left in op_strategy(), right in op_strategy()) {
    let left = build_replica(0, &left);
    let right = build_replica(1, &right);

    prop_assert_eq!(left.merge(&right), right.merge(&left));
  }

  #[test]
  fn merge_is_associative(left in op_strategy(), middle in op_strategy(), right in op_strategy()) {
    let left = build_replica(0, &left);
    let middle = build_replica(1, &middle);
    let right = build_replica(2, &right);

    prop_assert_eq!(left.merge(&middle.merge(&right)), left.merge(&middle).merge(&right));
  }

  #[test]
  fn merge_is_idempotent(ops in op_strategy()) {
    let map = build_replica(0, &ops);

    prop_assert_eq!(map.merge(&map), map.clone());
  }

  #[test]
  fn delta_merge_matches_full_state_merge(ops in op_strategy()) {
    let replica = build_replica(0, &ops);
    let full = ORMap::new().merge(&replica);
    let via_delta = match replica.delta() {
      | Some(delta) => ORMap::new().merge_delta(&delta),
      | None => ORMap::new(),
    };

    prop_assert_eq!(full, via_delta);
  }
}
