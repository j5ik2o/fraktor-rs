use core::ptr;

use super::super::{
  ConsistentHashingRoutingLogic, RandomRoutingLogic, RemoteRoutingLogic, RoundRobinRoutingLogic, Routee, RoutingLogic,
  SmallestMailboxRoutingLogic,
};
use crate::actor::{
  Pid,
  actor_ref::{ActorRef, NullSender},
  messaging::AnyMessage,
};

fn make_routee(id: u64) -> Routee {
  Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(id, 0), NullSender))
}

#[test]
fn round_robin_variant_delegates_select_and_select_index() {
  let logic = RemoteRoutingLogic::RoundRobin(RoundRobinRoutingLogic::new());
  let routees = [make_routee(1), make_routee(2)];
  let message = AnyMessage::new(());

  assert!(ptr::eq(logic.select(&message, &routees), &routees[0]));
  assert_eq!(logic.select_index(&routees), 1);
}

#[test]
fn smallest_mailbox_variant_delegates_select_and_select_index() {
  let logic = RemoteRoutingLogic::SmallestMailbox(SmallestMailboxRoutingLogic::new());
  let routees = [make_routee(1), make_routee(2)];
  let message = AnyMessage::new(());

  assert!(ptr::eq(logic.select(&message, &routees), &routees[0]));
  assert_eq!(logic.select_index(&routees), 0);
}

#[test]
fn random_variant_delegates_select_and_select_index() {
  let logic = RemoteRoutingLogic::Random(RandomRoutingLogic::new(1));
  let routees = [make_routee(1), make_routee(2), make_routee(3)];
  let message = AnyMessage::new(());

  let selected = logic.select(&message, &routees);
  assert!(routees.iter().any(|routee| ptr::eq(routee, selected)));
  assert!(logic.select_index(&routees) < routees.len());
}

#[test]
fn consistent_hashing_variant_delegates_select_and_select_index() {
  let logic = RemoteRoutingLogic::ConsistentHashing(ConsistentHashingRoutingLogic::new(|_| 42));
  let routees = [make_routee(1), make_routee(2), make_routee(3)];
  let message = AnyMessage::new("key");

  let selected = logic.select(&message, &routees);
  assert!(routees.iter().any(|routee| ptr::eq(routee, selected)));
  assert!(logic.select_index(&routees) < routees.len());
}

#[test]
fn empty_routees_return_no_routee_for_every_variant() {
  let routees = [];
  let message = AnyMessage::new(());
  let logics = [
    RemoteRoutingLogic::RoundRobin(RoundRobinRoutingLogic::new()),
    RemoteRoutingLogic::SmallestMailbox(SmallestMailboxRoutingLogic::new()),
    RemoteRoutingLogic::Random(RandomRoutingLogic::new(1)),
    RemoteRoutingLogic::ConsistentHashing(ConsistentHashingRoutingLogic::new(|_| 0)),
  ];

  for logic in logics {
    assert!(matches!(logic.select(&message, &routees), Routee::NoRoutee));
  }
}
