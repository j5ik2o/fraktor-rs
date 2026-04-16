use alloc::vec::Vec;

use super::super::{
  consistent_hashing_routing_logic::ConsistentHashingRoutingLogic, routee::Routee, routing_logic::RoutingLogic,
};
use crate::core::kernel::actor::{
  Pid,
  actor_ref::{ActorRef, NullSender},
  messaging::AnyMessage,
};

fn make_routee(id: u64) -> Routee {
  Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(id, 0), NullSender))
}

fn selected_pid(selected: &Routee) -> Pid {
  match selected {
    | Routee::ActorRef(actor_ref) => actor_ref.pid(),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}

fn hash_key_from_u32(message: &AnyMessage) -> u64 {
  u64::from(*message.downcast_ref::<u32>().expect("u32 message"))
}

#[test]
fn new_creates_logic() {
  // Given/When
  let _logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);

  // Then
  // construction succeeds without panic
}

#[test]
fn select_empty_routees_returns_no_routee() {
  // Given
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees: [Routee; 0] = [];
  let message = AnyMessage::new(7_u32);

  // When
  let selected = logic.select(&message, &routees);

  // Then
  assert!(matches!(selected, Routee::NoRoutee));
}

#[test]
fn select_same_hash_key_returns_same_routee() {
  // Given
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees = [make_routee(11), make_routee(22), make_routee(33)];
  let first = AnyMessage::new(5_u32);
  let second = AnyMessage::new(5_u32);

  // When
  let selected_first = logic.select(&first, &routees);
  let selected_second = logic.select(&second, &routees);

  // Then
  assert_eq!(selected_pid(selected_first), selected_pid(selected_second));
}

#[test]
fn select_is_stable_across_routee_order_changes() {
  // Given
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees = Vec::from([make_routee(11), make_routee(22), make_routee(33)]);
  let mut reordered = routees.clone();
  reordered.reverse();
  let message = AnyMessage::new(9_u32);

  // When
  let selected_original = logic.select(&message, &routees);
  let selected_reordered = logic.select(&message, &reordered);

  // Then
  assert_eq!(selected_pid(selected_original), selected_pid(selected_reordered));
}
