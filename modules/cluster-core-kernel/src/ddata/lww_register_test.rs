use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::ddata::{Key, LWWRegister, LWWRegisterKey, ReplicatedData, SelfUniqueAddress};

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

#[test]
fn new_uses_default_logical_clock() {
  let node = self_address(0);
  let register = LWWRegister::new(&node, "alpha");

  assert_eq!(register.value(), &"alpha");
  assert_eq!(register.timestamp(), 1);
  assert_eq!(register.updated_by(), node.unique_address());
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
  let updated = register.with_value(&second, "beta").expect("default clock must advance");

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
fn merge_can_model_first_write_wins_with_descending_timestamps() {
  let node = self_address(0);
  let first = LWWRegister::new_with_clock(&node, "alpha", LWWRegister::reverse_clock);
  let later_candidate =
    first.with_value_with_clock(&node, "beta", LWWRegister::reverse_clock).expect("reverse clock must move backwards");

  assert_eq!(first.merge(&later_candidate), first);
  assert_eq!(later_candidate.merge(&first), first);
}

#[test]
fn lww_register_key_is_typed() {
  let key: LWWRegisterKey<&'static str> = Key::new("register");

  assert_eq!(key.id(), "register");
}
