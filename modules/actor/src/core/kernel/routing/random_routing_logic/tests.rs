use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::{ActorRef, NullSender},
    messaging::AnyMessage,
  },
  routing::{RandomRoutingLogic, Routee, RoutingLogic},
};

fn make_routee(id: u64) -> Routee {
  Routee::ActorRef(ActorRef::new(Pid::new(id, 0), NullSender))
}

#[test]
fn new_creates_logic_with_seed() {
  // Given/When
  let _logic = RandomRoutingLogic::new(12345);

  // Then — construction succeeds without panic
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
    let idx_a = routees.iter().position(|r| core::ptr::eq(r, a)).unwrap();
    let idx_b = routees.iter().position(|r| core::ptr::eq(r, b)).unwrap();
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
