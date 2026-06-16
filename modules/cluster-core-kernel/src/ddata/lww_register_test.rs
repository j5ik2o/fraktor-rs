use alloc::collections::BTreeSet;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use proptest::prelude::*;

use crate::ddata::{Key, LWWRegister, LWWRegisterKey, RemovedNodePruning, ReplicatedData, SelfUniqueAddress};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Payload(&'static str);

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

fn register_at<T>(node: &SelfUniqueAddress, value: T, timestamp: i64) -> LWWRegister<T> {
  LWWRegister::new_with_clock(node, value, |_, _| timestamp)
}

fn pruning_node_for(removed_node: &UniqueAddress) -> UniqueAddress {
  UniqueAddress::new(removed_node.address().clone(), 0)
}

fn register_from_parts(node_index: usize, value: i64, timestamp: i64) -> LWWRegister<i64> {
  register_at(&self_address(node_index), value, timestamp)
}

fn clock_valid_register_from_parts(node_index: usize, timestamp: i64) -> LWWRegister<i64> {
  let value = timestamp.saturating_mul(4).saturating_add((node_index % 4) as i64);
  register_from_parts(node_index, value, timestamp)
}

#[test]
fn new_uses_default_clock_with_supplied_time() {
  let node = self_address(0);
  let register = LWWRegister::new(&node, "alpha", 1_700);

  assert_eq!(register.value(), &"alpha");
  assert_eq!(register.timestamp(), 1_700);
  assert_eq!(register.updated_by(), node.unique_address());
}

#[test]
fn new_uses_logical_floor_when_supplied_time_is_behind() {
  let node = self_address(0);
  let register = LWWRegister::new(&node, "alpha", 0);

  assert_eq!(register.timestamp(), 1);
}

#[test]
fn new_with_clock_uses_initial_timestamp_from_clock() {
  let node = self_address(0);
  let register = LWWRegister::new_with_clock(&node, "alpha", |current, value| {
    assert_eq!(current, 0);
    assert_eq!(value, &"alpha");
    100
  });

  assert_eq!(register.timestamp(), 100);
}

#[test]
fn with_value_replaces_value_writer_and_timestamp() {
  let first = self_address(0);
  let second = self_address(1);
  let register = register_at(&first, "alpha", 10);
  let updated = register.with_value(&second, "beta", 9).expect("default clock must advance");

  assert_eq!(updated.value(), &"beta");
  assert_eq!(updated.timestamp(), 11);
  assert_eq!(updated.updated_by(), second.unique_address());
}

#[test]
fn with_value_with_clock_uses_current_timestamp() {
  let first = self_address(0);
  let second = self_address(1);
  let register = register_at(&first, "alpha", 10);
  let updated = register
    .with_value_with_clock(&second, "beta", |current, value| {
      assert_eq!(current, 10);
      assert_eq!(value, &"beta");
      current + 1
    })
    .expect("clock must advance");

  assert_eq!(updated.timestamp(), 11);
}

#[test]
fn with_value_with_clock_rejects_same_writer_same_timestamp() {
  let node = self_address(0);
  let register = register_at(&node, "alpha", 10);

  assert_eq!(register.with_value_with_clock(&node, "beta", |current, _| current), None);
}

#[test]
fn default_clock_lets_unobserved_later_write_win_by_time() {
  let first = LWWRegister::new(&self_address(0), "alpha", 1_000);
  let second = LWWRegister::new(&self_address(1), "beta", 1_001);

  assert_eq!(first.merge(&second), second);
  assert_eq!(second.merge(&first), second);
}

#[test]
fn reverse_clock_lets_unobserved_earlier_write_win_by_time() {
  let first = LWWRegister::new_with_clock(&self_address(0), "alpha", |current, _| {
    LWWRegister::<&str>::reverse_clock(current, 1_000)
  });
  let second = LWWRegister::new_with_clock(&self_address(1), "beta", |current, _| {
    LWWRegister::<&str>::reverse_clock(current, 1_001)
  });

  assert_eq!(first.merge(&second), first);
  assert_eq!(second.merge(&first), first);
}

#[test]
fn merge_picks_larger_timestamp() {
  let first = register_at(&self_address(0), "alpha", 10);
  let second = register_at(&self_address(1), "beta", 11);

  assert_eq!(first.merge(&second), second);
  assert_eq!(second.merge(&first), second);
}

#[test]
fn merge_picks_lowest_node_order_when_timestamps_match() {
  let lower_node = register_at(&self_address(0), "alpha", 10);
  let higher_node = register_at(&self_address(1), "beta", 10);

  assert_eq!(higher_node.merge(&lower_node), lower_node);
  assert_eq!(lower_node.merge(&higher_node), lower_node);
}

#[test]
fn merge_keeps_same_write_when_writer_and_timestamp_match() {
  let node = self_address(0);
  let first = register_at(&node, "alpha", 10);
  let second = register_at(&node, "alpha", 10);

  assert_eq!(first.merge(&second), first);
  assert_eq!(second.merge(&first), second);
}

#[test]
fn merge_does_not_require_ordered_payload() {
  let older = register_at(&self_address(0), Payload("older"), 10);
  let newer = register_at(&self_address(1), Payload("newer"), 11);

  assert_eq!(older.merge(&newer), newer);
  assert_eq!(newer.merge(&older), newer);
}

#[test]
fn merge_can_model_first_write_wins_with_descending_timestamps() {
  let node = self_address(0);
  let first =
    LWWRegister::new_with_clock(&node, "alpha", |current, _| LWWRegister::<&str>::reverse_clock(current, 100));
  let later_candidate = first
    .with_value_with_clock(&node, "beta", |current, _| LWWRegister::<&str>::reverse_clock(current, 101))
    .expect("reverse clock must move backwards");

  assert_eq!(first.merge(&later_candidate), first);
  assert_eq!(later_candidate.merge(&first), first);
}

#[test]
fn prune_moves_writer_to_pruning_node() {
  let removed = self_address(0);
  let collapse = unique_address(1);
  let register = register_at(&removed, "alpha", 10);

  assert!(register.need_pruning_from(removed.unique_address()));

  let pruned = register.prune(removed.unique_address(), &collapse).expect("pruning is infallible");

  assert_eq!(pruned.updated_by(), &pruning_node_for(removed.unique_address()));
  assert_eq!(pruned.value(), &"alpha");
  assert_eq!(pruned.timestamp(), 10);
  assert!(!pruned.need_pruning_from(removed.unique_address()));
}

#[test]
fn prune_keeps_merge_order_independent_when_collapse_node_has_next_timestamp() {
  let removed = self_address(0);
  let collapse = self_address(1);
  let removed_register = register_at(&removed, "removed", 10);
  let collapse_register = register_at(&collapse, "collapse", 11);

  let pruned =
    removed_register.prune(removed.unique_address(), collapse.unique_address()).expect("pruning is infallible");

  assert_eq!(pruned.merge(&collapse_register), collapse_register);
  assert_eq!(collapse_register.merge(&pruned), collapse_register);
}

#[test]
fn prune_is_noop_when_collapse_into_same_node() {
  let removed = self_address(0);
  let register = register_at(&removed, "alpha", 10);

  let pruned =
    register.prune(removed.unique_address(), removed.unique_address()).expect("same-node pruning is infallible");

  assert_eq!(pruned, register);
  assert_eq!(pruned.timestamp(), 10);
}

#[test]
fn prune_keeps_max_timestamp_without_overflow() {
  let removed = self_address(0);
  let collapse = unique_address(1);
  let register = register_at(&removed, "alpha", i64::MAX);

  let pruned = register.prune(removed.unique_address(), &collapse).expect("pruning does not increment timestamp");

  assert_eq!(pruned.timestamp(), i64::MAX);
  assert_eq!(pruned.updated_by(), &pruning_node_for(removed.unique_address()));
}

#[test]
fn prune_leaves_register_written_by_other_node() {
  let writer = self_address(0);
  let removed = unique_address(2);
  let collapse = unique_address(1);
  let register = register_at(&writer, "alpha", 10);

  assert!(!register.need_pruning_from(&removed));

  let pruned = register.prune(&removed, &collapse).expect("pruning is infallible");

  assert_eq!(&pruned, &register);
}

#[test]
fn modified_by_nodes_reports_single_writer() {
  let writer = self_address(0);
  let register = register_at(&writer, "alpha", 10);

  assert_eq!(register.modified_by_nodes(), BTreeSet::from([writer.unique_address().clone()]));
}

#[test]
fn lww_register_key_is_typed() {
  let key: LWWRegisterKey<&'static str> = Key::new("register");

  assert_eq!(key.id(), "register");
}

proptest! {
  #[test]
  fn merge_is_commutative(
    left_node in 0_usize..4,
    left_timestamp in -20_i64..20,
    right_node in 0_usize..4,
    right_timestamp in -20_i64..20,
  ) {
    let left = clock_valid_register_from_parts(left_node, left_timestamp);
    let right = clock_valid_register_from_parts(right_node, right_timestamp);

    prop_assert_eq!(left.merge(&right), right.merge(&left));
  }

  #[test]
  fn merge_is_associative(
    left in (0_usize..4, -20_i64..20),
    middle in (0_usize..4, -20_i64..20),
    right in (0_usize..4, -20_i64..20),
  ) {
    let left = clock_valid_register_from_parts(left.0, left.1);
    let middle = clock_valid_register_from_parts(middle.0, middle.1);
    let right = clock_valid_register_from_parts(right.0, right.1);

    prop_assert_eq!(left.merge(&middle.merge(&right)), left.merge(&middle).merge(&right));
  }

  #[test]
  fn merge_is_idempotent(node in 0_usize..4, timestamp in -20_i64..20) {
    let register = clock_valid_register_from_parts(node, timestamp);

    prop_assert_eq!(register.merge(&register), register);
  }
}
