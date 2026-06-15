use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use crate::ddata::{
  CounterArithmeticError, Key, RemovedNodePruning, ReplicatedData, SelfUniqueAddress, VersionVector, VersionVectorKey,
  VersionVectorOrdering,
};

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

fn vector_from_entries(entries: &[(usize, u64)]) -> VersionVector {
  VersionVector::from_entries(entries.iter().map(|(index, version)| (unique_address(*index), *version)))
}

fn entry_strategy() -> impl Strategy<Value = Vec<(usize, u64)>> {
  prop::collection::vec((0_usize..4, 0_u64..20), 0..24)
}

#[test]
fn increment_advances_local_node_version() {
  let node = self_address(0);

  let vector = VersionVector::new().increment(&node).expect("version must fit");
  let advanced = vector.increment(&node).expect("version must fit");

  assert_eq!(advanced.version_at(node.unique_address()), 2);
  assert!(advanced.contains_node(node.unique_address()));
}

#[test]
fn from_entries_keeps_highest_non_zero_version_per_node() {
  let vector = VersionVector::from_entries([(unique_address(0), 1), (unique_address(0), 3), (unique_address(1), 0)]);

  assert_eq!(vector.version_at(&unique_address(0)), 3);
  assert_eq!(vector.version_at(&unique_address(1)), 0);
  assert_eq!(vector.len(), 1);
}

#[test]
fn compare_reports_same_before_after_and_concurrent() {
  let empty = VersionVector::new();
  let left = vector_from_entries(&[(0, 2), (1, 1)]);
  let after = vector_from_entries(&[(0, 3), (1, 1), (2, 1)]);
  let concurrent = vector_from_entries(&[(0, 1), (1, 3)]);

  assert_eq!(left.compare(&left), VersionVectorOrdering::Same);
  assert!(empty.is_before(&left));
  assert!(after.is_after(&left));
  assert!(left.is_concurrent(&concurrent));
}

#[test]
fn merge_uses_per_node_maximum() {
  let left = vector_from_entries(&[(0, 2), (1, 5)]);
  let right = vector_from_entries(&[(0, 3), (2, 4)]);

  let merged = left.merge(&right);

  assert_eq!(merged.version_at(&unique_address(0)), 3);
  assert_eq!(merged.version_at(&unique_address(1)), 5);
  assert_eq!(merged.version_at(&unique_address(2)), 4);
}

#[test]
fn entries_are_returned_in_node_order() {
  let vector = vector_from_entries(&[(2, 7), (0, 5)]);
  let entries = vector.entries().map(|(node, version)| (node.clone(), version)).collect::<Vec<_>>();

  assert_eq!(entries, vec![(unique_address(0), 5), (unique_address(2), 7)]);
}

#[test]
fn pruning_collapses_removed_node_into_active_node() {
  let removed = unique_address(0);
  let collapse_into = unique_address(1);
  let vector = vector_from_entries(&[(0, 7), (1, 2)]);

  let pruned = vector.prune(&removed, &collapse_into).expect("collapse version must fit");

  assert_eq!(pruned.version_at(&removed), 0);
  assert_eq!(pruned.version_at(&collapse_into), 8);
  assert!(!pruned.need_pruning_from(&removed));
  assert!(pruned.need_pruning_from(&collapse_into));
}

#[test]
fn pruning_same_node_removes_without_reinserting() {
  let removed = unique_address(0);
  let vector = vector_from_entries(&[(0, 7)]);

  let pruned = vector.prune(&removed, &removed).expect("same node pruning must not overflow");

  assert!(pruned.is_empty());
}

#[test]
fn pruning_cleanup_removes_removed_node_only() {
  let removed = unique_address(0);
  let survivor = unique_address(1);
  let vector = vector_from_entries(&[(0, 7), (1, 2)]);

  let cleaned = vector.pruning_cleanup(&removed);

  assert_eq!(cleaned.version_at(&removed), 0);
  assert_eq!(cleaned.version_at(&survivor), 2);
}

#[test]
fn prune_detects_collapse_version_overflow() {
  let removed = unique_address(0);
  let collapse_into = unique_address(1);
  let vector = VersionVector::from_entries([(removed.clone(), u64::MAX), (collapse_into.clone(), 1)]);

  assert_eq!(vector.prune(&removed, &collapse_into), Err(CounterArithmeticError::Overflow));
}

#[test]
fn version_vector_key_is_typed() {
  let key: VersionVectorKey = Key::new("versions");

  assert_eq!(key.id(), "versions");
}

proptest! {
  #[test]
  fn merge_is_commutative(left_entries in entry_strategy(), right_entries in entry_strategy()) {
    let left = vector_from_entries(&left_entries);
    let right = vector_from_entries(&right_entries);

    prop_assert_eq!(left.merge(&right), right.merge(&left));
  }

  #[test]
  fn merge_is_associative(
    left_entries in entry_strategy(),
    middle_entries in entry_strategy(),
    right_entries in entry_strategy(),
  ) {
    let left = vector_from_entries(&left_entries);
    let middle = vector_from_entries(&middle_entries);
    let right = vector_from_entries(&right_entries);

    prop_assert_eq!(left.merge(&middle.merge(&right)), left.merge(&middle).merge(&right));
  }

  #[test]
  fn merge_is_idempotent(entries in entry_strategy()) {
    let vector = vector_from_entries(&entries);

    prop_assert_eq!(vector.merge(&vector), vector);
  }
}
