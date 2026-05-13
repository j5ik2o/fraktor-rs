use alloc::{vec, vec::Vec};
use core::ptr;

use super::super::{round_robin_routing_logic::RoundRobinRoutingLogic, routee::Routee, routing_logic::RoutingLogic};
use crate::actor::{
  Pid,
  actor_ref::{ActorRef, NullSender},
  messaging::AnyMessage,
};

fn make_routee(id: u64) -> Routee {
  Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(id, 0), NullSender))
}

#[test]
fn new_creates_logic() {
  // Given/When
  let _logic = RoundRobinRoutingLogic::new();

  // Then — construction succeeds without panic
}

#[test]
fn select_cycles_through_routees() {
  // Given
  let logic = RoundRobinRoutingLogic::new();
  let routees = [make_routee(1), make_routee(2), make_routee(3)];
  let message = AnyMessage::new(42_u32);

  // When — select 6 times
  let mut selected_indices = Vec::new();
  for _ in 0..6 {
    let selected = logic.select(&message, &routees);
    let idx = routees.iter().position(|r| ptr::eq(r, selected)).expect("selected routee not found in routees");
    selected_indices.push(idx);
  }

  // Then — round-robin pattern: [0, 1, 2, 0, 1, 2]
  assert_eq!(selected_indices, vec![0, 1, 2, 0, 1, 2]);
}

#[test]
fn select_single_routee_always_returns_it() {
  // Given
  let logic = RoundRobinRoutingLogic::new();
  let routees = [make_routee(1)];
  let message = AnyMessage::new(42_u32);

  // When/Then — always returns the single routee
  for _ in 0..3 {
    let selected = logic.select(&message, &routees);
    assert!(ptr::eq(selected, &routees[0]));
  }
}

#[test]
fn select_wraps_cleanly_after_counter_overflow() {
  // Given
  let logic = RoundRobinRoutingLogic::with_initial_counter(usize::MAX);
  let routees = [make_routee(1), make_routee(2)];
  let message = AnyMessage::new(42_u32);

  // When
  let first = logic.select(&message, &routees);
  let second = logic.select(&message, &routees);

  // Then
  assert!(ptr::eq(first, &routees[1]));
  assert!(ptr::eq(second, &routees[0]));
}

#[test]
fn select_empty_routees_returns_no_routee() {
  // Given
  let logic = RoundRobinRoutingLogic::new();
  let routees: [Routee; 0] = [];
  let message = AnyMessage::new(42_u32);

  // When
  let selected = logic.select(&message, &routees);

  // Then
  assert!(matches!(selected, Routee::NoRoutee));
}
