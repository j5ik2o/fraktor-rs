use super::*;
use crate::core::kernel::actor::{Pid, actor_ref::ActorRef};

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
  let payload = fraktor_utils_core_rs::core::sync::ArcShared::new(42_u32)
    as fraktor_utils_core_rs::core::sync::ArcShared<dyn core::any::Any + Send + Sync>;
  let message = AnyMessage::from_erased(payload, None, true);
  assert!(message.is_control());
}

#[test]
fn from_erased_preserves_control_flag_false() {
  let payload = fraktor_utils_core_rs::core::sync::ArcShared::new(42_u32)
    as fraktor_utils_core_rs::core::sync::ArcShared<dyn core::any::Any + Send + Sync>;
  let message = AnyMessage::from_erased(payload, None, false);
  assert!(!message.is_control());
}
