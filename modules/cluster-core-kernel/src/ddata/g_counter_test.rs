use alloc::collections::BTreeMap;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use crate::ddata::{
  CounterArithmeticError, DeltaReplicatedData, GCounter, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
  SelfUniqueAddress,
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

fn counter_from_ops(ops: &[(usize, u64)]) -> GCounter {
  let mut counter = GCounter::new();
  for (index, amount) in ops {
    counter = counter.increment(&self_address(*index), *amount).expect("small generated increments must fit");
  }
  counter
}

fn op_strategy() -> impl Strategy<Value = Vec<(usize, u64)>> {
  prop::collection::vec((0_usize..4, 0_u64..20), 0..24)
}

#[test]
fn increment_adds_to_self_node_slot() {
  let node = self_address(0);

  let counter = GCounter::new().increment(&node, 5).expect("increment must fit");

  assert_eq!(counter.value(), Ok(5));
  assert!(counter.need_pruning_from(node.unique_address()));
}

#[test]
fn merge_uses_per_node_maximum() {
  let node = self_address(0);
  let left = GCounter::new().increment(&node, 3).expect("increment must fit");
  let right = GCounter::new().increment(&node, 5).expect("increment must fit");

  assert_eq!(left.merge(&right).value(), Ok(5));
}

#[test]
fn delta_reset_and_zero_follow_full_state_contract() {
  let node = self_address(0);
  let counter = GCounter::new().increment(&node, 5).expect("increment must fit");
  let delta = counter.delta().expect("increment must create delta");

  assert_eq!(GCounter::new().merge_delta(&delta).value(), Ok(5));
  assert_eq!(counter.reset_delta().delta(), None);
  assert_eq!(delta.zero(), GCounter::new());
}

#[test]
fn merge_delta_preserves_local_delta() {
  let local = GCounter::new().increment(&self_address(0), 3).expect("increment must fit");
  let remote = GCounter::new().increment(&self_address(1), 5).expect("increment must fit");
  let remote_delta = remote.delta().expect("remote increment must create delta");

  let merged = local.merge_delta(&remote_delta);
  let remaining_delta = merged.delta().expect("local delta must remain");

  assert_eq!(merged.value(), Ok(8));
  assert_eq!(remaining_delta.value(), Ok(3));
}

#[test]
fn pruning_moves_removed_node_contribution() {
  let removed = self_address(0);
  let collapse_into = self_address(1);
  let counter = GCounter::new()
    .increment(&removed, 5)
    .expect("increment must fit")
    .increment(&collapse_into, 2)
    .expect("increment must fit");

  let pruned = counter.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");

  assert_eq!(pruned.value(), Ok(7));
  assert!(!pruned.need_pruning_from(removed.unique_address()));
  assert!(pruned.need_pruning_from(collapse_into.unique_address()));
}

#[test]
fn pruning_same_node_removes_without_reinserting() {
  let removed = self_address(0);
  let counter = GCounter::new().increment(&removed, 5).expect("increment must fit");

  let pruned = counter.prune(removed.unique_address(), removed.unique_address()).expect("pruning must fit");

  assert_eq!(pruned.value(), Ok(0));
  assert!(!pruned.need_pruning_from(removed.unique_address()));
}

#[test]
fn increment_detects_overflow() {
  let node = self_address(0);
  let mut state = BTreeMap::new();
  state.insert(node.unique_address().clone(), u128::MAX);
  let counter = GCounter::from_parts(state, BTreeMap::new());

  assert_eq!(counter.increment(&node, 1), Err(CounterArithmeticError::Overflow));
}

#[test]
fn value_detects_overflow() {
  let mut state = BTreeMap::new();
  state.insert(unique_address(0), u128::MAX);
  state.insert(unique_address(1), 1);
  let counter = GCounter::from_parts(state, BTreeMap::new());

  assert_eq!(counter.value(), Err(CounterArithmeticError::Overflow));
}

#[test]
fn prune_detects_overflow() {
  let removed = unique_address(0);
  let collapse_into = unique_address(1);
  let mut state = BTreeMap::new();
  state.insert(removed.clone(), u128::MAX);
  state.insert(collapse_into.clone(), 1);
  let counter = GCounter::from_parts(state, BTreeMap::new());

  assert_eq!(counter.prune(&removed, &collapse_into), Err(CounterArithmeticError::Overflow));
}

proptest! {
  #[test]
  fn merge_is_commutative(left_ops in op_strategy(), right_ops in op_strategy()) {
    let left = counter_from_ops(&left_ops);
    let right = counter_from_ops(&right_ops);

    prop_assert_eq!(left.merge(&right), right.merge(&left));
  }

  #[test]
  fn merge_is_associative(left_ops in op_strategy(), middle_ops in op_strategy(), right_ops in op_strategy()) {
    let left = counter_from_ops(&left_ops);
    let middle = counter_from_ops(&middle_ops);
    let right = counter_from_ops(&right_ops);

    prop_assert_eq!(left.merge(&middle.merge(&right)), left.merge(&middle).merge(&right));
  }

  #[test]
  fn merge_is_idempotent(ops in op_strategy()) {
    let value = counter_from_ops(&ops);

    prop_assert_eq!(value.merge(&value), value);
  }

  #[test]
  fn merge_delta_matches_full_state_merge(base_ops in op_strategy(), delta_ops in op_strategy()) {
    let base = counter_from_ops(&base_ops);
    let full_with_delta = counter_from_ops(&delta_ops);
    let delta = full_with_delta.delta().unwrap_or_else(GCounter::new);

    prop_assert_eq!(base.merge_delta(&delta), base.merge(&full_with_delta));
  }
}
