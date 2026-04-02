use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::{ActorRef, NullSender},
    messaging::AnyMessage,
  },
};

use super::super::{random_routing_logic::RandomRoutingLogic, routee::Routee, routing_logic::RoutingLogic};

fn make_routee(id: u64) -> Routee {
  Routee::ActorRef(ActorRef::new(Pid::new(id, 0), NullSender))
}

#[test]
fn select_returns_valid_routee() {
  // Given
  let logic = RandomRoutingLogic::new(42);
  let routees = [make_routee(1), make_routee(2), make_routee(3)];
  let message = AnyMessage::new(42_u32);

  // When/Then — all selections must point to one of the routees
  for _ in 0..10 {
    let selected = logic.select(&message, &routees);
    let found = routees.iter().any(|r| core::ptr::eq(r, selected));
    assert!(found, "selected routee must be one of the provided routees");
  }
}

#[test]
fn select_with_same_seed_produces_same_sequence() {
  // Given
  let logic_a = RandomRoutingLogic::new(99);
  let logic_b = RandomRoutingLogic::new(99);
  let routees = [make_routee(1), make_routee(2), make_routee(3)];
  let message = AnyMessage::new(42_u32);

  // When
  let mut seq_a = alloc::vec::Vec::new();
  let mut seq_b = alloc::vec::Vec::new();
  for _ in 0..10 {
    let a = logic_a.select(&message, &routees);
    let b = logic_b.select(&message, &routees);
    let idx_a = match routees.iter().position(|r| core::ptr::eq(r, a)) {
      | Some(index) => index,
      | None => panic!("selected routee for logic_a not found in routees"),
    };
    let idx_b = match routees.iter().position(|r| core::ptr::eq(r, b)) {
      | Some(index) => index,
      | None => panic!("selected routee for logic_b not found in routees"),
    };
    seq_a.push(idx_a);
    seq_b.push(idx_b);
  }

  // Then
  assert_eq!(seq_a, seq_b, "same seed must produce same selection sequence");
}

#[test]
fn select_empty_routees_returns_no_routee() {
  // Given
  let logic = RandomRoutingLogic::new(42);
  let routees: [Routee; 0] = [];
  let message = AnyMessage::new(42_u32);

  // When
  let selected = logic.select(&message, &routees);

  // Then
  assert!(matches!(selected, Routee::NoRoutee));
}
