use super::*;
use crate::actor::{
  Pid,
  actor_ref::ActorRef,
  messaging::{DeadLetterSuppression, NotInfluenceReceiveTimeout, PossiblyHarmful},
};

struct NonInfluencingTick;

impl NotInfluenceReceiveTimeout for NonInfluencingTick {}

struct SuppressedTick;

impl DeadLetterSuppression for SuppressedTick {}

struct HarmfulTick;

impl PossiblyHarmful for HarmfulTick {}

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

#[test]
fn dead_letter_suppressed_sets_suppression_flag() {
  let message = AnyMessage::dead_letter_suppressed(SuppressedTick);
  assert!(message.is_dead_letter_suppressed());
  assert!(!message.is_possibly_harmful());

  let view = message.as_view();
  assert!(view.dead_letter_suppressed());
  assert!(!view.possibly_harmful());
}

#[test]
fn possibly_harmful_sets_remote_safety_flag() {
  let message = AnyMessage::possibly_harmful(HarmfulTick);
  assert!(message.is_possibly_harmful());
  assert!(!message.is_dead_letter_suppressed());

  let cloned = message.clone();
  assert!(cloned.is_possibly_harmful());
}

#[test]
fn from_parts_with_flags_preserves_marker_flags() {
  let payload = ArcShared::new(42_u32) as ArcShared<dyn Any + Send + Sync>;
  let message = AnyMessage::from_parts_with_flags(payload, None, false, true, true, true);

  assert!(message.is_not_influence_receive_timeout());
  assert!(message.is_dead_letter_suppressed());
  assert!(message.is_possibly_harmful());
}

#[test]
fn into_parts_returns_payload_sender_and_all_flags() {
  let payload = ArcShared::new(42_u32) as ArcShared<dyn Any + Send + Sync>;
  let sender = ActorRef::null();
  let message = AnyMessage::from_parts_with_flags(payload, Some(sender.clone()), true, true, true, true);

  let (payload, sender_out, is_control, not_influence, dead_letter_suppressed, possibly_harmful) = message.into_parts();

  assert_eq!(payload.downcast_ref::<u32>(), Some(&42_u32));
  assert_eq!(sender_out.as_ref(), Some(&sender));
  assert!(is_control);
  assert!(not_influence);
  assert!(dead_letter_suppressed);
  assert!(possibly_harmful);
}
