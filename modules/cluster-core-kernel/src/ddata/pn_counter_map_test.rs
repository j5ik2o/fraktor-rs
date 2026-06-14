use alloc::collections::BTreeMap;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use super::{
  super::{g_counter::GCounter, pn_counter::PNCounter},
  PNCounterMap,
};
use crate::ddata::{
  CounterArithmeticError, DeltaReplicatedData, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
  RequiresCausalDeliveryOfDeltas, SelfUniqueAddress,
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

#[derive(Debug, Clone)]
enum MapOp {
  Increment { index: usize, key: u8, amount: u64 },
  Decrement { index: usize, key: u8, amount: u64 },
  Remove { key: u8 },
}

fn map_from_ops(ops: &[MapOp]) -> PNCounterMap<u8> {
  let mut map = PNCounterMap::new();
  for op in ops {
    map = match op {
      | MapOp::Increment { index, key, amount } => {
        map.increment(&self_address(*index), *key, *amount).expect("small generated increments must fit")
      },
      | MapOp::Decrement { index, key, amount } => {
        map.decrement(&self_address(*index), *key, *amount).expect("small generated decrements must fit")
      },
      | MapOp::Remove { key } => map.remove(key),
    };
  }
  map
}

fn op_strategy_for_nodes(nodes: impl Strategy<Value = usize> + Clone + 'static) -> impl Strategy<Value = Vec<MapOp>> {
  prop::collection::vec(
    prop_oneof![
      (nodes.clone(), 0_u8..4, 0_u64..20).prop_map(|(index, key, amount)| MapOp::Increment { index, key, amount }),
      (nodes, 0_u8..4, 0_u64..20).prop_map(|(index, key, amount)| MapOp::Decrement { index, key, amount }),
      (0_u8..4).prop_map(|key| MapOp::Remove { key }),
    ],
    0..24,
  )
}

fn op_strategy() -> impl Strategy<Value = Vec<MapOp>> {
  op_strategy_for_nodes(0_usize..4)
}

fn g_counter_with_slot(index: usize, value: u128) -> GCounter {
  let mut state = BTreeMap::new();
  state.insert(unique_address(index), value);
  GCounter::from_parts(state, BTreeMap::new())
}

fn assert_requires_causal_delivery<T: RequiresCausalDeliveryOfDeltas>() {}

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
fn entries_surface_reports_visible_counter_values() {
  let map = PNCounterMap::new()
    .increment(&self_address(0), 1, 7)
    .expect("increment must fit")
    .decrement(&self_address(1), 1, 2)
    .expect("decrement must fit")
    .increment(&self_address(2), 2, 11)
    .expect("increment must fit");

  let mut expected = BTreeMap::new();
  expected.insert(1, 5);
  expected.insert(2, 11);

  assert_eq!(map.entries(), Ok(expected));
  assert!(map.contains_key(&1));
  assert_eq!(map.len(), 2);
  assert!(!map.is_empty());
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
fn remove_drops_observed_key_after_full_state_merge() {
  let left = PNCounterMap::new()
    .increment(&self_address(0), 1, 1)
    .expect("increment must fit")
    .increment(&self_address(0), 2, 3)
    .expect("increment must fit")
    .increment(&self_address(0), 3, 2)
    .expect("increment must fit");
  let right = PNCounterMap::new().increment(&self_address(1), 3, 5).expect("increment must fit");
  let merged = left.merge(&right);

  let removed = merged.remove(&2);
  let merged_after_remove = merged.merge(&removed);

  let mut expected = BTreeMap::new();
  expected.insert(1, 1);
  expected.insert(3, 7);

  assert_eq!(merged_after_remove.entries(), Ok(expected));
  assert_eq!(merged_after_remove.get(&2), Ok(None));
}

#[test]
fn remove_keeps_concurrent_full_state_update() {
  let left = PNCounterMap::new()
    .increment(&self_address(0), 1, 1)
    .expect("increment must fit")
    .increment(&self_address(0), 2, 3)
    .expect("increment must fit")
    .increment(&self_address(0), 3, 2)
    .expect("increment must fit");
  let right = PNCounterMap::new().increment(&self_address(1), 3, 5).expect("increment must fit");
  let merged = left.merge(&right);

  let removed = merged.remove(&2);
  let concurrent = merged.increment(&self_address(1), 2, 10).expect("increment must fit");

  assert_eq!(removed.merge(&concurrent).get(&2), Ok(Some(10)));
}

#[test]
fn remove_then_recreate_ignores_stale_observed_value() {
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let recreated = original.remove(&1).increment(&self_address(0), 1, 1).expect("increment must fit");

  assert_eq!(recreated.get(&1), Ok(Some(1)));
  assert_eq!(recreated.merge(&original).get(&1), Ok(Some(1)));
}

#[test]
fn remove_then_recreate_delta_ignores_stale_observed_value() {
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let recreated_delta = original
    .reset_delta()
    .remove(&1)
    .increment(&self_address(0), 1, 1)
    .expect("increment must fit")
    .delta()
    .expect("remove and recreate must create delta");

  assert_eq!(original.merge_delta(&recreated_delta).get(&1), Ok(Some(1)));
}

#[test]
fn delta_reset_and_zero_follow_full_state_contract() {
  let map = PNCounterMap::new().increment(&self_address(0), 1, 7).expect("increment must fit");
  let delta = map.delta().expect("increment must create delta");

  assert_eq!(PNCounterMap::new().merge_delta(&delta).get(&1), Ok(Some(7)));
  assert_eq!(map.reset_delta().delta(), None);
  assert_eq!(delta.zero(), PNCounterMap::new());
}

#[test]
fn map_delta_requires_causal_delivery() {
  assert_requires_causal_delivery::<PNCounterMap<u8>>();
}

#[test]
fn merge_delta_preserves_local_delta() {
  let local = PNCounterMap::new().increment(&self_address(0), 1, 7).expect("increment must fit");
  let remote = PNCounterMap::new().decrement(&self_address(1), 2, 2).expect("decrement must fit");
  let remote_delta = remote.delta().expect("remote update must create delta");

  let merged = local.merge_delta(&remote_delta);
  let remaining_delta = merged.delta().expect("local delta must remain");

  assert_eq!(merged.get(&1), Ok(Some(7)));
  assert_eq!(merged.get(&2), Ok(Some(-2)));
  assert_eq!(remaining_delta.get(&1), Ok(Some(7)));
  assert_eq!(remaining_delta.get(&2), Ok(None));
}

#[test]
fn remove_delta_drops_observed_key() {
  let left = PNCounterMap::new()
    .increment(&self_address(0), 1, 1)
    .expect("increment must fit")
    .increment(&self_address(0), 2, 3)
    .expect("increment must fit")
    .increment(&self_address(0), 3, 2)
    .expect("increment must fit");
  let right = PNCounterMap::new().increment(&self_address(1), 3, 5).expect("increment must fit");
  let merged = left.merge(&right);

  let removed = merged.reset_delta().remove(&2);
  let remove_delta = removed.delta().expect("remove must create delta");

  assert_eq!(merged.merge_delta(&remove_delta).get(&2), Ok(None));
}

#[test]
fn remove_drops_covered_pending_add_delta() {
  let added = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let removed = added.remove(&1);
  let remove_delta = removed.delta().expect("remove must create delta");

  assert_eq!(remove_delta.get(&1), Ok(None));
  assert_eq!(PNCounterMap::new().merge(&remove_delta).get(&1), Ok(None));
  assert_eq!(added.merge_delta(&remove_delta).get(&1), Ok(None));
}

#[test]
fn remove_delta_keeps_concurrent_update_delta() {
  let left = PNCounterMap::new()
    .increment(&self_address(0), 1, 1)
    .expect("increment must fit")
    .increment(&self_address(0), 2, 3)
    .expect("increment must fit")
    .increment(&self_address(0), 3, 2)
    .expect("increment must fit");
  let right = PNCounterMap::new().increment(&self_address(1), 3, 5).expect("increment must fit");
  let merged = left.merge(&right);

  let removed = merged.reset_delta().remove(&2);
  let concurrent = merged.reset_delta().increment(&self_address(1), 2, 10).expect("increment must fit");
  let concurrent_delta = concurrent.delta().expect("concurrent update must create delta");

  assert_eq!(removed.merge_delta(&concurrent_delta).get(&2), Ok(Some(10)));
}

#[test]
fn remote_remove_drops_covered_local_delta() {
  let local = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let remove_delta = local.reset_delta().remove(&1).delta().expect("remove must create delta");

  let merged = local.merge_delta(&remove_delta);

  assert_eq!(merged.get(&1), Ok(None));
  assert_eq!(merged.delta(), None);
}

#[test]
fn full_state_remove_drops_covered_local_delta() {
  let local = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let remote_remove = local.reset_delta().remove(&1);

  let merged = local.merge(&remote_remove);

  assert_eq!(merged.get(&1), Ok(None));
  assert_eq!(merged.delta(), None);
}

#[test]
fn full_state_merge_prunes_local_delta_with_merged_removed_value_prefix() {
  let node = unique_address(0);
  let removed_prefix_3 = PNCounter::new().increment(&self_address(0), 3).expect("increment must fit").reset_delta();
  let removed_prefix_5 = PNCounter::new().increment(&self_address(0), 5).expect("increment must fit").reset_delta();
  let pending_value_8 = PNCounter::new().increment(&self_address(0), 8).expect("increment must fit").reset_delta();

  let mut local_removed_values = BTreeMap::new();
  local_removed_values.insert(1, removed_prefix_5);
  let mut local_delta = BTreeMap::new();
  local_delta.insert(1, pending_value_8);
  let mut local_delta_key_dots = BTreeMap::new();
  local_delta_key_dots.insert(node.clone(), 2);
  let mut local_delta_dots = BTreeMap::new();
  local_delta_dots.insert(1, local_delta_key_dots);
  let local = PNCounterMap {
    entries:              BTreeMap::new(),
    dots:                 BTreeMap::new(),
    removed_dots:         BTreeMap::new(),
    removed_values:       local_removed_values,
    delta:                local_delta,
    delta_dots:           local_delta_dots,
    delta_removed_dots:   BTreeMap::new(),
    delta_removed_values: BTreeMap::new(),
  };

  let mut remote_removed_key_dots = BTreeMap::new();
  remote_removed_key_dots.insert(node, 1);
  let mut remote_removed_dots = BTreeMap::new();
  remote_removed_dots.insert(1, remote_removed_key_dots);
  let mut remote_removed_values = BTreeMap::new();
  remote_removed_values.insert(1, removed_prefix_3);
  let remote = PNCounterMap {
    entries:              BTreeMap::new(),
    dots:                 BTreeMap::new(),
    removed_dots:         remote_removed_dots,
    removed_values:       remote_removed_values,
    delta:                BTreeMap::new(),
    delta_dots:           BTreeMap::new(),
    delta_removed_dots:   BTreeMap::new(),
    delta_removed_values: BTreeMap::new(),
  };

  let merged_delta = local.merge(&remote).delta().expect("remaining delta must be visible");

  assert_eq!(merged_delta.get(&1), Ok(Some(3)));
}

#[test]
fn full_state_merge_keeps_recreated_local_delta_after_reset_delta() {
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let recreated = original.remove(&1).reset_delta().increment(&self_address(0), 1, 8).expect("increment must fit");
  let remote_remove = original.reset_delta().remove(&1);

  let merged = recreated.merge(&remote_remove);
  let remaining_delta = merged.delta().expect("recreated local value must remain in delta");

  assert_eq!(merged.get(&1), Ok(Some(8)));
  assert_eq!(remaining_delta.get(&1), Ok(Some(8)));
}

#[test]
fn merge_delta_keeps_recreated_local_delta_after_reset_delta() {
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let recreated = original.remove(&1).reset_delta().increment(&self_address(0), 1, 8).expect("increment must fit");
  let remove_delta = original.reset_delta().remove(&1).delta().expect("remove must create delta");

  let merged = recreated.merge_delta(&remove_delta);
  let remaining_delta = merged.delta().expect("recreated local value must remain in delta");

  assert_eq!(merged.get(&1), Ok(Some(8)));
  assert_eq!(remaining_delta.get(&1), Ok(Some(8)));
}

#[test]
fn remove_tombstone_participates_in_equality() {
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let removed = original.remove(&1);

  assert_ne!(removed, PNCounterMap::new());
  assert_eq!(removed.merge(&original).get(&1), Ok(None));
  assert_eq!(PNCounterMap::new().merge(&original).get(&1), Ok(Some(5)));
}

#[test]
fn merge_excludes_tombstoned_counter_slots() {
  let node_0_entry = PNCounterMap::new().increment(&self_address(0), 1, 1).expect("increment must fit");
  let node_1_entry = PNCounterMap::new().increment(&self_address(1), 1, 2).expect("increment must fit");
  let node_0_removed = node_0_entry.remove(&1);

  let left_associated = node_0_entry.merge(&node_1_entry.merge(&node_0_removed));
  let right_associated = node_0_entry.merge(&node_1_entry).merge(&node_0_removed);

  assert_eq!(left_associated.get(&1), Ok(Some(2)));
  assert_eq!(right_associated.get(&1), Ok(Some(2)));
  assert_eq!(left_associated, right_associated);
}

#[test]
fn merge_subtracts_removed_same_node_prefix_from_later_dot() {
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let removed = original.remove(&1);
  let stale_later_dot = original.increment(&self_address(0), 1, 3).expect("increment must fit");

  assert_eq!(removed.merge(&stale_later_dot).get(&1), Ok(Some(3)));
}

#[test]
fn merge_subtracts_equal_removed_prefix_from_later_dot() {
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let removed = original.remove(&1);
  let stale_later_dot = original.decrement(&self_address(0), 1, 2).expect("decrement must fit");

  assert_eq!(removed.merge(&stale_later_dot).get(&1), Ok(Some(-2)));
}

#[test]
fn repeated_remove_replaces_same_node_removed_prefix() {
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let recreated = original.remove(&1).increment(&self_address(0), 1, 3).expect("increment must fit");
  let removed_again = recreated.remove(&1);
  let concurrent = recreated.increment(&self_address(0), 1, 2).expect("increment must fit");

  assert_eq!(removed_again.merge(&concurrent).get(&1), Ok(Some(2)));
}

#[test]
fn merge_preserves_local_delta() {
  let local = PNCounterMap::new().increment(&self_address(0), 1, 7).expect("increment must fit");
  let remote = PNCounterMap::new().decrement(&self_address(1), 2, 2).expect("decrement must fit");

  let merged = local.merge(&remote);
  let remaining_delta = merged.delta().expect("local delta must remain");

  assert_eq!(merged.get(&1), Ok(Some(7)));
  assert_eq!(merged.get(&2), Ok(Some(-2)));
  assert_eq!(remaining_delta.get(&1), Ok(Some(7)));
  assert_eq!(remaining_delta.get(&2), Ok(None));
}

#[test]
fn merge_resets_inserted_entry_nested_delta() {
  let remote = PNCounterMap::new().increment(&self_address(0), 1, 7).expect("increment must fit");

  let merged = PNCounterMap::new().merge(&remote);
  let entry_delta = merged.entries.get(&1).and_then(DeltaReplicatedData::delta);

  assert_eq!(merged.get(&1), Ok(Some(7)));
  assert_eq!(entry_delta, None);
}

#[test]
fn zero_update_does_not_create_absent_key() {
  let map = PNCounterMap::new().increment(&self_address(0), 1, 0).expect("increment must fit");

  assert_eq!(map.get(&1), Ok(None));
}

#[test]
fn pruning_cleanup_drops_emptied_delta_entries() {
  let removed = self_address(0);
  let map = PNCounterMap::new().increment(&removed, 1, 7).expect("increment must fit");

  let cleaned = map.pruning_cleanup(removed.unique_address());

  assert_eq!(cleaned.delta(), None);
}

#[test]
fn pruning_cleanup_drops_emptied_removed_dots_entries() {
  let removed = self_address(0);
  let map = PNCounterMap::new().increment(&removed, 1, 7).expect("increment must fit").remove(&1);

  let cleaned = map.pruning_cleanup(removed.unique_address());

  assert!(cleaned.removed_dots.is_empty());
  assert_eq!(cleaned, PNCounterMap::new());
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
fn prune_preserves_unrelated_local_delta() {
  let removed = self_address(0);
  let collapse_into = self_address(1);
  let unrelated = self_address(2);
  let map = PNCounterMap::new()
    .increment(&removed, 1, 5)
    .expect("increment must fit")
    .reset_delta()
    .increment(&unrelated, 2, 3)
    .expect("increment must fit");

  let pruned = map.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");
  let remaining_delta = pruned.delta().expect("unrelated delta must remain");

  assert_eq!(pruned.get(&1), Ok(Some(5)));
  assert_eq!(pruned.get(&2), Ok(Some(3)));
  assert_eq!(remaining_delta.get(&1), Ok(Some(5)));
  assert_eq!(remaining_delta.get(&2), Ok(Some(3)));
}

#[test]
fn prune_records_entry_collapse_contribution_in_delta() {
  let removed = self_address(0);
  let collapse_into = self_address(1);
  let map = PNCounterMap::new().increment(&removed, 1, 5).expect("increment must fit").reset_delta();

  let pruned = map.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");
  let delta = pruned.delta().expect("collapse contribution must be replicated");

  assert_eq!(PNCounterMap::new().merge_delta(&delta).get(&1), Ok(Some(5)));
}

#[test]
fn prune_drops_entry_when_pruned_dots_become_empty() {
  let removed = self_address(0);
  let map = PNCounterMap::new().increment(&removed, 1, 5).expect("increment must fit").reset_delta();

  let pruned = map.prune(removed.unique_address(), removed.unique_address()).expect("pruning must fit");

  assert_eq!(pruned.get(&1), Ok(None));
  assert!(!pruned.contains_key(&1));
  assert_eq!(pruned.len(), 0);
  assert!(pruned.is_empty());
}

#[test]
fn prune_preserves_pending_remove_delta() {
  let removed = self_address(2);
  let collapse_into = self_address(3);
  let original = PNCounterMap::new().increment(&self_address(0), 1, 5).expect("increment must fit");
  let pending_remove = original.reset_delta().remove(&1);

  let pruned =
    pending_remove.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");
  let delta = pruned.delta().expect("pending remove must remain in delta");

  assert_eq!(original.merge_delta(&delta).get(&1), Ok(None));
}

#[test]
fn prune_emits_stable_tombstone_delta() {
  let removed = self_address(0);
  let collapse_into = self_address(2);
  let original = PNCounterMap::new().increment(&removed, 1, 5).expect("increment must fit");
  let stable_tombstone = original.remove(&1).reset_delta();

  let pruned_tombstone =
    stable_tombstone.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");
  let tombstone_delta = pruned_tombstone.delta().expect("pruned tombstone must be replicated");
  let pruned_entry =
    original.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");

  assert_eq!(pruned_entry.merge_delta(&tombstone_delta).get(&1), Ok(None));
}

#[test]
fn prune_does_not_retarget_removed_dot_tombstones() {
  let removed = self_address(0);
  let collapse_into = self_address(2);
  let removed_entry = PNCounterMap::new().increment(&removed, 1, 5).expect("increment must fit");
  let tombstone = removed_entry.remove(&1);

  let pruned = tombstone.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");
  let concurrent = PNCounterMap::new().increment(&collapse_into, 1, 4).expect("increment must fit");

  assert!(!pruned.need_pruning_from(removed.unique_address()));
  assert_eq!(pruned.merge(&concurrent).get(&1), Ok(Some(4)));
}

#[test]
fn prune_avoids_collapse_dot_collision_with_target_tombstone() {
  let removed = self_address(0);
  let collapse_into = self_address(2);
  let target_removed = PNCounterMap::new().increment(&collapse_into, 1, 7).expect("increment must fit").remove(&1);
  let visible_removed_node_entry = PNCounterMap::new().increment(&removed, 1, 5).expect("increment must fit");
  let map = target_removed.merge(&visible_removed_node_entry);

  let pruned = map.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning must fit");

  assert_eq!(pruned.get(&1), Ok(Some(5)));
  assert_eq!(pruned.merge(&pruned).get(&1), Ok(Some(5)));
}

#[test]
fn pruned_tombstone_suppresses_pruned_removed_node_entry() {
  let removed = self_address(0);
  let collapse_into = self_address(2);
  let original = PNCounterMap::new().increment(&removed, 1, 5).expect("increment must fit");
  let pruned_tombstone = original
    .remove(&1)
    .prune(removed.unique_address(), collapse_into.unique_address())
    .expect("pruning tombstone must fit");
  let pruned_entry =
    original.prune(removed.unique_address(), collapse_into.unique_address()).expect("pruning entry must fit");

  assert_eq!(pruned_tombstone.merge(&pruned_entry).get(&1), Ok(None));
}

#[test]
fn get_propagates_nested_counter_overflow() {
  let counter = PNCounter::from_parts(g_counter_with_slot(0, u128::MAX), GCounter::new());
  let mut entries = BTreeMap::new();
  entries.insert(1, counter);
  let map = PNCounterMap {
    entries,
    dots: BTreeMap::new(),
    removed_dots: BTreeMap::new(),
    removed_values: BTreeMap::new(),
    delta: BTreeMap::new(),
    delta_dots: BTreeMap::new(),
    delta_removed_dots: BTreeMap::new(),
    delta_removed_values: BTreeMap::new(),
  };

  assert_eq!(map.get(&1), Err(CounterArithmeticError::Overflow));
  assert_eq!(map.entries(), Err(CounterArithmeticError::Overflow));
}

proptest! {
  #[test]
  fn merge_delta_matches_full_state_merge(
    base_ops in op_strategy_for_nodes(0_usize..2),
    delta_ops in op_strategy_for_nodes(2_usize..4),
  ) {
    let base = map_from_ops(&base_ops);
    let full_with_delta = map_from_ops(&delta_ops);
    let delta = full_with_delta.delta().unwrap_or_else(PNCounterMap::new);

    prop_assert_eq!(base.merge_delta(&delta), base.merge(&full_with_delta));
  }

  #[test]
  fn merge_is_commutative(left_ops in op_strategy_for_nodes(0_usize..2), right_ops in op_strategy_for_nodes(2_usize..4)) {
    let left = map_from_ops(&left_ops);
    let right = map_from_ops(&right_ops);

    prop_assert_eq!(left.merge(&right), right.merge(&left));
  }

  #[test]
  fn merge_is_associative(
    left_ops in op_strategy_for_nodes(0_usize..1),
    middle_ops in op_strategy_for_nodes(1_usize..2),
    right_ops in op_strategy_for_nodes(2_usize..3),
  ) {
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
