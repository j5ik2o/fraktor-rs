use alloc::collections::BTreeSet;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use crate::ddata::{DeltaReplicatedData, ORSet, RemovedNodePruning, ReplicatedData, SelfUniqueAddress};

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

fn build_replica(node_index: usize, ops: &[(bool, u8)]) -> ORSet<u8> {
  let node = self_address(node_index);
  let mut set = ORSet::new();
  for (is_add, element) in ops {
    set = if *is_add { set.add(&node, *element) } else { set.remove(element) };
  }
  set
}

fn op_strategy() -> impl Strategy<Value = Vec<(bool, u8)>> {
  prop::collection::vec((any::<bool>(), 0_u8..6), 0..20)
}

#[test]
fn add_makes_element_visible() {
  let node = self_address(0);
  let set = ORSet::new().add(&node, "x");

  assert!(set.contains(&"x"));
  assert_eq!(set.elements(), BTreeSet::from(["x"]));
  assert_eq!(set.len(), 1);
}

#[test]
fn remove_hides_observed_element() {
  let node = self_address(0);
  let set = ORSet::new().add(&node, "x").remove(&"x");

  assert!(!set.contains(&"x"));
  assert!(set.is_empty());
}

#[test]
fn concurrent_add_wins_over_remove() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let shared = ORSet::new().add(&node_a, "x");

  let removed = shared.remove(&"x");
  let concurrently_added = shared.add(&node_b, "x");

  assert!(removed.merge(&concurrently_added).contains(&"x"));
  assert!(concurrently_added.merge(&removed).contains(&"x"));
}

#[test]
fn readd_after_remove_survives_merge() {
  let node = self_address(0);
  let shared = ORSet::new().add(&node, "x");

  let removed = shared.remove(&"x");
  let readded = shared.add(&node, "x");

  assert!(removed.merge(&readded).contains(&"x"));
}

#[test]
fn add_preserves_existing_element_dots_after_merge() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let merged = ORSet::new().add(&node_a, "x").merge(&ORSet::new().add(&node_b, "x"));

  let added = merged.add(&node_a, "x");
  let dots = added.dots_for(&"x").expect("element stays visible");

  assert_eq!(dots.version_at(node_a.unique_address()), 2);
  assert_eq!(dots.version_at(node_b.unique_address()), 1);
}

#[test]
fn pure_remove_wins_without_concurrent_add() {
  let node = self_address(0);
  let shared = ORSet::new().add(&node, "x");

  let removed = shared.remove(&"x");

  assert!(!removed.merge(&shared).contains(&"x"));
  assert!(!shared.merge(&removed).contains(&"x"));
}

#[test]
fn merge_is_order_independent() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let left = ORSet::new().add(&node_a, "x").add(&node_a, "y");
  let right = ORSet::new().add(&node_b, "y").add(&node_b, "z");

  assert_eq!(left.merge(&right), right.merge(&left));
}

#[test]
fn clear_empties_but_keeps_history() {
  let node = self_address(0);
  let set = ORSet::new().add(&node, "x").add(&node, "y");

  let cleared = set.clear();

  assert!(cleared.is_empty());
  assert!(!cleared.merge(&set).contains(&"x"));
}

#[test]
fn delta_application_matches_full_merge() {
  let node = self_address(0);
  let added = ORSet::new().add(&node, "x");
  let first_delta = added.delta().expect("local change produces a delta");

  let removed = added.reset_delta().remove(&"x");
  let second_delta = removed.delta().expect("local change produces a delta");

  let replicated = ORSet::new().merge_delta(&first_delta).merge_delta(&second_delta);

  assert_eq!(replicated, removed);
  assert!(!replicated.contains(&"x"));
}

#[test]
fn delta_is_absent_without_local_change() {
  let node = self_address(0);
  let settled = ORSet::new().add(&node, "x").reset_delta();

  assert!(settled.delta().is_none());
}

#[test]
fn pruning_preserves_surviving_elements() {
  let node_a = self_address(0);
  let node_b = self_address(1);
  let removed = unique_address(0);
  let collapse = unique_address(1);
  let set = ORSet::new().add(&node_a, "x").add(&node_b, "y");

  assert!(set.need_pruning_from(&removed));

  let pruned = set.prune(&removed, &collapse).expect("set pruning is infallible");

  assert!(!pruned.need_pruning_from(&removed));
  assert!(pruned.contains(&"x"));
  assert!(pruned.contains(&"y"));
}

#[test]
fn pruning_preserves_existing_element_dots() {
  let removed_node = self_address(0);
  let survivor_node = self_address(1);
  let collapse = unique_address(2);
  let merged = ORSet::new().add(&removed_node, "x").merge(&ORSet::new().add(&survivor_node, "x"));

  let pruned = merged.prune(removed_node.unique_address(), &collapse).expect("set pruning is infallible");
  let dots = pruned.dots_for(&"x").expect("element stays visible");

  assert_eq!(dots.version_at(survivor_node.unique_address()), 1);
  assert_eq!(dots.version_at(&collapse), 2);
}

#[test]
fn pruning_cleanup_drops_element_when_all_dots_belong_to_removed_node() {
  let removed_node = self_address(0);
  let set = ORSet::new().add(&removed_node, "x");

  let cleaned = set.pruning_cleanup(removed_node.unique_address());

  assert!(!cleaned.contains(&"x"));
  assert!(cleaned.is_empty());
  assert!(!cleaned.need_pruning_from(removed_node.unique_address()));
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
    let set = build_replica(0, &ops);

    prop_assert_eq!(set.merge(&set), set.clone());
  }

  #[test]
  fn delta_merge_matches_full_state_merge(ops in op_strategy()) {
    let replica = build_replica(0, &ops);
    let full = ORSet::new().merge(&replica);
    let via_delta = match replica.delta() {
      | Some(delta) => ORSet::new().merge_delta(&delta),
      | None => ORSet::new(),
    };

    prop_assert_eq!(full, via_delta);
  }

  #[test]
  fn pruning_preserves_visible_elements(ops in op_strategy(), removed_index in 0_usize..4, collapse_index in 0_usize..4) {
    let set = build_replica(0, &ops);
    let removed = unique_address(removed_index);
    let collapse = unique_address(collapse_index);
    let before = set.elements();

    let pruned = match set.prune(&removed, &collapse) {
      | Ok(pruned) => pruned,
      | Err(never) => match never {},
    };

    prop_assert_eq!(pruned.elements(), before);
    prop_assert!(removed == collapse || !pruned.need_pruning_from(&removed));
  }
}
