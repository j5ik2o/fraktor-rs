use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use crate::ddata::{DeltaReplicatedData, LWWMap, RemovedNodePruning, ReplicatedData, SelfUniqueAddress};

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

fn build_replica(node_index: usize, ops: &[(bool, u8, i64)]) -> LWWMap<u8, i64> {
  let node = self_address(node_index);
  let mut map = LWWMap::new();
  for (is_put, key, value) in ops {
    map = if *is_put { map.put_with_clock(&node, *key, *value, |timestamp, _| timestamp + 1) } else { map.remove(key) };
  }
  map
}

fn op_strategy() -> impl Strategy<Value = Vec<(bool, u8, i64)>> {
  prop::collection::vec((any::<bool>(), 0_u8..3, -20_i64..20), 0..15)
}

#[test]
fn put_then_get_returns_value() {
  let node = self_address(0);
  let map = LWWMap::new().put(&node, 1_u8, 100_i64, 1_000);

  assert_eq!(map.get(&1), Some(&100));
  assert!(map.contains_key(&1));
  assert_eq!(map.len(), 1);
}

#[test]
fn concurrent_put_with_greater_timestamp_wins() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let left = LWWMap::new().put_with_clock(&node_a, 1_u8, 10_i64, |_, _| 5);
  let right = LWWMap::new().put_with_clock(&node_b, 1_u8, 20_i64, |_, _| 9);

  assert_eq!(left.merge(&right).get(&1), Some(&20));
  assert_eq!(right.merge(&left).get(&1), Some(&20));
}

#[test]
fn concurrent_put_with_equal_timestamp_breaks_tie_by_node() {
  let lower_node = self_address(0);
  let higher_node = self_address(1);
  let lower = LWWMap::new().put_with_clock(&lower_node, 1_u8, 10_i64, |_, _| 7);
  let higher = LWWMap::new().put_with_clock(&higher_node, 1_u8, 20_i64, |_, _| 7);

  assert_eq!(lower.merge(&higher).get(&1), Some(&10));
  assert_eq!(higher.merge(&lower).get(&1), Some(&10));
}

#[test]
fn remove_hides_entry() {
  let node = self_address(0);
  let map = LWWMap::new().put(&node, 1_u8, 100_i64, 1_000).remove(&1);

  assert!(!map.contains_key(&1));
  assert!(map.is_empty());
}

#[test]
fn remove_absent_key_is_noop_without_delta() {
  let map = LWWMap::<u8, i64>::new();

  let removed = map.remove(&1);

  assert_eq!(removed, map);
  assert!(removed.delta().is_none());
}

#[test]
fn rejected_clock_update_does_not_readd_removed_key() {
  let node = self_address(0);
  let shared = LWWMap::new().put_with_clock(&node, 1_u8, 10_i64, |_, _| 5);
  let removed = shared.remove(&1);

  let rejected = shared.put_with_clock(&node, 1_u8, 20_i64, |timestamp, _| timestamp);

  assert_eq!(rejected.get(&1), Some(&10));
  assert!(!removed.merge(&rejected).contains_key(&1));
  assert!(!rejected.merge(&removed).contains_key(&1));
}

#[test]
fn lower_timestamp_put_keeps_existing_value_and_delta_clean() {
  let node = self_address(0);
  let existing = LWWMap::new().put_with_clock(&node, 1_u8, 10_i64, |_, _| 10).reset_delta();

  let rejected = existing.put_with_clock(&node, 1_u8, 20_i64, |timestamp, _| timestamp - 1);

  assert_eq!(rejected.get(&1), Some(&10));
  assert_eq!(rejected, existing);
  assert!(rejected.delta().is_none());
}

#[test]
fn merge_is_order_independent() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let left = LWWMap::new().put(&node_a, 1_u8, 1_i64, 1_000);
  let right = LWWMap::new().put(&node_b, 2_u8, 2_i64, 1_000);

  assert_eq!(left.merge(&right), right.merge(&left));
}

#[test]
fn delta_application_matches_full_merge() {
  let node = self_address(0);
  let added = LWWMap::new().put(&node, 1_u8, 10_i64, 1_000);
  let first_delta = added.delta().expect("local change produces a delta");

  let removed = added.reset_delta().remove(&1);
  let second_delta = removed.delta().expect("local change produces a delta");

  let replicated = LWWMap::new().merge_delta(&first_delta).merge_delta(&second_delta);

  assert_eq!(replicated, removed);
  assert!(!replicated.contains_key(&1));
}

#[test]
fn pruning_preserves_value() {
  let node_a = self_address(0);
  let removed = unique_address(0);
  let collapse = unique_address(1);
  let map = LWWMap::new().put(&node_a, 1_u8, 42_i64, 1_000);

  assert!(map.need_pruning_from(&removed));

  let pruned = map.prune(&removed, &collapse).expect("lww map pruning is infallible");

  assert!(!pruned.need_pruning_from(&removed));
  assert_eq!(pruned.get(&1), Some(&42));
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
    let full = LWWMap::new().merge(&replica);
    let via_delta = match replica.delta() {
      | Some(delta) => LWWMap::new().merge_delta(&delta),
      | None => LWWMap::new(),
    };

    prop_assert_eq!(full, via_delta);
  }
}
