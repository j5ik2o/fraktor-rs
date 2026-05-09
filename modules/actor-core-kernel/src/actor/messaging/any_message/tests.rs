use super::*;
use crate::actor::{Pid, actor_ref::ActorRef, messaging::NotInfluenceReceiveTimeout};

struct NonInfluencingTick;

impl NotInfluenceReceiveTimeout for NonInfluencingTick {}

#[test]
fn stores_payload_and_sender() {
  let mut message: AnyMessage = AnyMessage::new(5_u32);
  assert_eq!(message.payload().downcast_ref::<u32>(), Some(&5));

  let sender = ActorRef::null();
  message = message.with_sender(sender.clone());
  assert_eq!(message.sender(), Some(&sender));

  let view = message.as_view();
  assert_eq!(view.downcast_ref::<u32>(), Some(&5));
  assert!(view.sender().is_some());
  assert_eq!(view.sender().unwrap().pid(), Pid::new(0, 0));
}

#[test]
fn new_message_is_not_control() {
  let message = AnyMessage::new(42_u32);
  assert!(!message.is_control());
}

#[test]
fn control_message_is_marked_as_control() {
  let message = AnyMessage::control(42_u32);
  assert!(message.is_control());
  assert_eq!(message.payload().downcast_ref::<u32>(), Some(&42));
}

#[test]
fn control_flag_preserved_through_clone() {
  let original = AnyMessage::control(99_u32);
  let cloned = original.clone();
  assert!(cloned.is_control());
}

#[test]
fn control_message_supports_sender() {
  let sender = ActorRef::null();
  let message = AnyMessage::control(7_u32).with_sender(sender.clone());
  assert!(message.is_control());
  assert_eq!(message.sender(), Some(&sender));
}

#[test]
fn from_erased_preserves_control_flag_true() {
  let payload = ArcShared::new(42_u32) as ArcShared<dyn Any + Send + Sync>;
  let message = AnyMessage::from_erased(payload, None, true, false);
  assert!(message.is_control());
}

#[test]
fn from_erased_preserves_control_flag_false() {
  let payload = ArcShared::new(42_u32) as ArcShared<dyn Any + Send + Sync>;
  let message = AnyMessage::from_erased(payload, None, false, false);
  assert!(!message.is_control());
}

#[test]
fn new_message_is_not_flagged_as_not_influence() {
  let message = AnyMessage::new(NonInfluencingTick);
  assert!(!message.is_not_influence_receive_timeout());
}

#[test]
fn not_influence_sets_receive_timeout_flag() {
  let message = AnyMessage::not_influence(NonInfluencingTick);
  assert!(message.is_not_influence_receive_timeout());
  assert!(!message.is_control());
}

#[test]
fn not_influence_flag_is_preserved_on_clone() {
  let original = AnyMessage::not_influence(NonInfluencingTick);
  let cloned = original.clone();
  assert!(cloned.is_not_influence_receive_timeout());
}

#[test]
fn view_exposes_not_influence_flag() {
  let message = AnyMessage::not_influence(NonInfluencingTick);
  let view = message.as_view();
  assert!(view.not_influence_receive_timeout());
  assert!(!view.is_control());
}

#[test]
fn regular_view_reports_not_influence_flag_as_false() {
  let message = AnyMessage::new(NonInfluencingTick);
  let view = message.as_view();
  assert!(!view.not_influence_receive_timeout());
}
