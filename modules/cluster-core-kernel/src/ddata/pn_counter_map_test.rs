use alloc::collections::BTreeMap;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use super::{
  super::{g_counter::GCounter, pn_counter::PNCounter},
  PNCounterMap,
};
use crate::ddata::{CounterArithmeticError, RemovedNodePruning, ReplicatedData, SelfUniqueAddress};

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

fn map_from_ops(ops: &[(usize, u8, bool, u64)]) -> PNCounterMap<u8> {
  let mut map = PNCounterMap::new();
  for (index, key, increment, amount) in ops {
    let node = self_address(*index);
    map = if *increment { map.increment(&node, *key, *amount) } else { map.decrement(&node, *key, *amount) }
      .expect("small generated increments must fit");
  }
  map
}

fn op_strategy() -> impl Strategy<Value = Vec<(usize, u8, bool, u64)>> {
  prop::collection::vec((0_usize..4, 0_u8..4, any::<bool>(), 0_u64..20), 0..24)
}

fn g_counter_with_slot(index: usize, value: u128) -> GCounter {
  let mut state = BTreeMap::new();
  state.insert(unique_address(index), value);
  GCounter::from_parts(state, BTreeMap::new())
}

#[test]
fn increment_decrement_and_get_are_key_scoped() {
  let map = PNCounterMap::new()
    .increment(&self_address(0), 1, 7)
    .expect("increment must fit")
    .decrement(&self_address(1), 1, 2)
    .expect("decrement must fit")
    .increment(&self_address(2), 2, 11)
    .expect("increment must fit");

  assert_eq!(map.get(&1), Ok(Some(5)));
  assert_eq!(map.get(&2), Ok(Some(11)));
  assert_eq!(map.get(&3), Ok(None));
}

#[test]
fn merge_unions_keys_and_merges_shared_counters() {
  let left = PNCounterMap::new()
    .increment(&self_address(0), 1, 7)
    .expect("increment must fit")
    .increment(&self_address(1), 2, 3)
    .expect("increment must fit");
  let right = PNCounterMap::new()
    .decrement(&self_address(2), 1, 2)
    .expect("decrement must fit")
    .increment(&self_address(3), 3, 4)
    .expect("increment must fit");

  let merged = left.merge(&right);

  assert_eq!(merged.get(&1), Ok(Some(5)));
  assert_eq!(merged.get(&2), Ok(Some(3)));
  assert_eq!(merged.get(&3), Ok(Some(4)));
}

#[test]
fn zero_update_does_not_create_absent_key() {
  let map = PNCounterMap::new().increment(&self_address(0), 1, 0).expect("increment must fit");

  assert_eq!(map.get(&1), Ok(None));
}

#[test]
fn pruning_delegates_to_entries() {
  let removed = self_address(0);
  let collapse_into = self_address(1);
  let map = PNCounterMap::new()
    .increment(&removed, 1, 5)
    .expect("increment must fit")
    .increment(&collapse_into, 1, 2)
    .expect("increment must fit");

  let pruned = map.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");

  assert_eq!(pruned.get(&1), Ok(Some(7)));
  assert!(!pruned.need_pruning_from(removed.unique_address()));
}

#[test]
fn get_propagates_nested_counter_overflow() {
  let counter = PNCounter::from_parts(g_counter_with_slot(0, u128::MAX), GCounter::new());
  let mut entries = BTreeMap::new();
  entries.insert(1, counter);
  let map = PNCounterMap { entries };

  assert_eq!(map.get(&1), Err(CounterArithmeticError::Overflow));
}

proptest! {
  #[test]
  fn merge_is_commutative(left_ops in op_strategy(), right_ops in op_strategy()) {
    let left = map_from_ops(&left_ops);
    let right = map_from_ops(&right_ops);

    prop_assert_eq!(left.merge(&right), right.merge(&left));
  }

  #[test]
  fn merge_is_associative(left_ops in op_strategy(), middle_ops in op_strategy(), right_ops in op_strategy()) {
    let left = map_from_ops(&left_ops);
    let middle = map_from_ops(&middle_ops);
    let right = map_from_ops(&right_ops);

    prop_assert_eq!(left.merge(&middle.merge(&right)), left.merge(&middle).merge(&right));
  }

  #[test]
  fn merge_is_idempotent(ops in op_strategy()) {
    let value = map_from_ops(&ops);

    prop_assert_eq!(value.merge(&value), value);
  }
}
