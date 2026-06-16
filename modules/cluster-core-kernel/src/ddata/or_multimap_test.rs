use alloc::collections::BTreeSet;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use crate::ddata::{DeltaReplicatedData, ORMultiMap, RemovedNodePruning, ReplicatedData, SelfUniqueAddress};

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

fn build_replica(node_index: usize, ops: &[(bool, u8, u8)]) -> ORMultiMap<u8, u8> {
  let node = self_address(node_index);
  let mut map = ORMultiMap::new();
  for (is_add, key, element) in ops {
    map = if *is_add { map.add_binding(&node, *key, *element) } else { map.remove_binding(&node, key, element) };
  }
  map
}

fn op_strategy() -> impl Strategy<Value = Vec<(bool, u8, u8)>> {
  prop::collection::vec((any::<bool>(), 0_u8..3, 0_u8..4), 0..15)
}

#[test]
fn add_binding_makes_element_visible() {
  let node = self_address(0);
  let map = ORMultiMap::new().add_binding(&node, 1_u8, 10_u8).add_binding(&node, 1_u8, 20_u8);

  assert_eq!(map.get(&1), Some(BTreeSet::from([10, 20])));
  assert!(map.contains_key(&1));
}

#[test]
fn remove_binding_drops_only_that_element() {
  let node = self_address(0);
  let map =
    ORMultiMap::new().add_binding(&node, 1_u8, 10_u8).add_binding(&node, 1_u8, 20_u8).remove_binding(&node, &1, &10);

  assert_eq!(map.get(&1), Some(BTreeSet::from([20])));
}

#[test]
fn removing_last_binding_removes_key() {
  let node = self_address(0);
  let map = ORMultiMap::new().add_binding(&node, 1_u8, 10_u8).remove_binding(&node, &1, &10);

  assert!(!map.contains_key(&1));
  assert_eq!(map.get(&1), None);
  assert!(map.is_empty());
}

#[test]
fn removing_absent_binding_does_not_readd_removed_key() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let shared = ORMultiMap::new().add_binding(&node_a, 1_u8, 10_u8);
  let removed = shared.remove_binding(&node_a, &1, &10);

  let no_op_remove = shared.remove_binding(&node_b, &1, &20);

  assert_eq!(no_op_remove.get(&1), Some(BTreeSet::from([10])));
  assert_eq!(removed.merge(&no_op_remove).get(&1), None);
  assert_eq!(no_op_remove.merge(&removed).get(&1), None);
}

#[test]
fn concurrent_add_and_remove_binding_keeps_element() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let shared = ORMultiMap::new().add_binding(&node_a, 1_u8, 10_u8);

  let removed = shared.remove_binding(&node_a, &1, &10);
  let concurrently_added = shared.add_binding(&node_b, 1_u8, 10_u8);

  assert_eq!(removed.merge(&concurrently_added).get(&1), Some(BTreeSet::from([10])));
}

#[test]
fn merge_is_order_independent() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let left = ORMultiMap::new().add_binding(&node_a, 1_u8, 10_u8);
  let right = ORMultiMap::new().add_binding(&node_b, 2_u8, 20_u8);

  assert_eq!(left.merge(&right), right.merge(&left));
}

#[test]
fn delta_application_matches_full_merge() {
  let node = self_address(0);
  let added = ORMultiMap::new().add_binding(&node, 1_u8, 10_u8);
  let first_delta = added.delta().expect("local change produces a delta");

  let extended = added.reset_delta().add_binding(&node, 1_u8, 20_u8);
  let second_delta = extended.delta().expect("local change produces a delta");

  let replicated = ORMultiMap::new().merge_delta(&first_delta).merge_delta(&second_delta);

  assert_eq!(replicated, extended);
  assert_eq!(replicated.get(&1), Some(BTreeSet::from([10, 20])));
}

#[test]
fn pruning_preserves_bindings() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let removed = unique_address(0);
  let collapse = unique_address(1);
  let map = ORMultiMap::new().add_binding(&node_a, 1_u8, 10_u8).add_binding(&node_b, 2_u8, 20_u8);

  assert!(map.need_pruning_from(&removed));

  let pruned = map.prune(&removed, &collapse).expect("set pruning is infallible");

  assert!(!pruned.need_pruning_from(&removed));
  assert_eq!(pruned.get(&1), Some(BTreeSet::from([10])));
  assert_eq!(pruned.get(&2), Some(BTreeSet::from([20])));
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
    let full = ORMultiMap::new().merge(&replica);
    let via_delta = match replica.delta() {
      | Some(delta) => ORMultiMap::new().merge_delta(&delta),
      | None => ORMultiMap::new(),
    };

    prop_assert_eq!(full, via_delta);
  }
}
