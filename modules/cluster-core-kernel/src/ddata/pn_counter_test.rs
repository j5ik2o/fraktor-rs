use alloc::collections::BTreeMap;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use super::super::g_counter::GCounter;
use crate::ddata::{
  CounterArithmeticError, DeltaReplicatedData, PNCounter, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
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

fn counter_from_ops(ops: &[(usize, bool, u64)]) -> PNCounter {
  let mut counter = PNCounter::new();
  for (index, increment, amount) in ops {
    let node = self_address(*index);
    counter = if *increment { counter.increment(&node, *amount) } else { counter.decrement(&node, *amount) }
      .expect("small generated increments must fit");
  }
  counter
}

fn op_strategy() -> impl Strategy<Value = Vec<(usize, bool, u64)>> {
  prop::collection::vec((0_usize..4, any::<bool>(), 0_u64..20), 0..24)
}

fn g_counter_with_slot(index: usize, value: u128) -> GCounter {
  let mut state = BTreeMap::new();
  state.insert(unique_address(index), value);
  GCounter::from_parts(state, BTreeMap::new())
}

#[test]
fn increment_and_decrement_update_signed_value() {
  let inc_node = self_address(0);
  let dec_node = self_address(1);

  let counter = PNCounter::new()
    .increment(&inc_node, 7)
    .expect("increment must fit")
    .decrement(&dec_node, 2)
    .expect("decrement must fit");

  assert_eq!(counter.value(), Ok(5));
}

#[test]
fn merge_combines_components_independently() {
  let left = PNCounter::new().increment(&self_address(0), 7).expect("increment must fit");
  let right = PNCounter::new().decrement(&self_address(1), 2).expect("decrement must fit");

  assert_eq!(left.merge(&right).value(), Ok(5));
}

#[test]
fn delta_reset_and_zero_follow_full_state_contract() {
  let counter = PNCounter::new()
    .increment(&self_address(0), 7)
    .expect("increment must fit")
    .decrement(&self_address(1), 2)
    .expect("decrement must fit");
  let delta = counter.delta().expect("updates must create delta");

  assert_eq!(PNCounter::new().merge_delta(&delta).value(), Ok(5));
  assert_eq!(counter.reset_delta().delta(), None);
  assert_eq!(delta.zero(), PNCounter::new());
}

#[test]
fn merge_delta_preserves_local_delta() {
  let local = PNCounter::new().increment(&self_address(0), 7).expect("increment must fit");
  let remote = PNCounter::new().decrement(&self_address(1), 2).expect("decrement must fit");
  let remote_delta = remote.delta().expect("remote update must create delta");

  let merged = local.merge_delta(&remote_delta);
  let remaining_delta = merged.delta().expect("local delta must remain");

  assert_eq!(merged.value(), Ok(5));
  assert_eq!(remaining_delta.value(), Ok(7));
}

#[test]
fn pruning_delegates_to_components() {
  let removed = self_address(0);
  let collapse_into = self_address(1);
  let counter = PNCounter::new()
    .increment(&removed, 5)
    .expect("increment must fit")
    .increment(&collapse_into, 2)
    .expect("increment must fit");

  let pruned = counter.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");

  assert_eq!(pruned.value(), Ok(7));
  assert!(!pruned.need_pruning_from(removed.unique_address()));
}

#[test]
fn value_detects_positive_overflow() {
  let counter = PNCounter::from_parts(g_counter_with_slot(0, u128::MAX), GCounter::new());

  assert_eq!(counter.value(), Err(CounterArithmeticError::Overflow));
}

#[test]
fn value_allows_i128_minimum() {
  let counter = PNCounter::from_parts(GCounter::new(), g_counter_with_slot(0, 1_u128 << 127));

  assert_eq!(counter.value(), Ok(i128::MIN));
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
    let delta = full_with_delta.delta().unwrap_or_default();

    prop_assert_eq!(base.merge_delta(&delta), base.merge(&full_with_delta));
  }
}
