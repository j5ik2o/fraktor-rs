use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::{ActorRef, NullSender},
  },
};

use super::super::{routee::Routee, router_command::RouterCommand};

#[test]
fn get_routees_variant_is_constructible() {
  // Given/When
  let cmd = RouterCommand::GetRoutees;

  // Then
  assert!(matches!(cmd, RouterCommand::GetRoutees));
}

#[test]
fn add_routee_variant_is_constructible() {
  // Given
  let routee = Routee::ActorRef(ActorRef::new(Pid::new(1, 0), NullSender));

  // When
  let cmd = RouterCommand::AddRoutee(routee);

  // Then
  assert!(matches!(cmd, RouterCommand::AddRoutee(Routee::ActorRef(_))));
}

#[test]
fn remove_routee_variant_is_constructible() {
  // Given
  let routee = Routee::ActorRef(ActorRef::new(Pid::new(2, 0), NullSender));

  // When
  let cmd = RouterCommand::RemoveRoutee(routee);

  // Then
  assert!(matches!(cmd, RouterCommand::RemoveRoutee(Routee::ActorRef(_))));
}

#[test]
fn adjust_pool_size_variant_is_constructible() {
  // Given/When
  let cmd = RouterCommand::AdjustPoolSize(5);

  // Then
  assert!(matches!(cmd, RouterCommand::AdjustPoolSize(5)));
}

#[test]
fn adjust_pool_size_negative_value() {
  // Given/When
  let cmd = RouterCommand::AdjustPoolSize(-3);

  // Then
  assert!(matches!(cmd, RouterCommand::AdjustPoolSize(-3)));
}
