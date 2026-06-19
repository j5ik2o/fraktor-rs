use core::any::Any;

use crate::actor::{
  actor_ref::ActorRef,
  messaging::{AnyMessage, AnyMessageView},
};

#[test]
fn downcasts_payload() {
  let message: AnyMessage = AnyMessage::new(10_i32);
  let view: AnyMessageView<'_> = message.as_view();
  assert_eq!(view.downcast_ref::<i32>(), Some(&10));
}

#[test]
fn carries_sender_reference() {
  let sender: ActorRef = ActorRef::null();
  let message = AnyMessage::new("ping").with_sender(sender.clone());
  let view = message.as_view();
  assert!(matches!(view.sender(), Some(r) if r == &sender));
}

#[test]
fn new_view_defaults_all_envelope_flags_to_false() {
  let payload: &(dyn Any + Send + Sync + 'static) = &42_u32;
  let view = AnyMessageView::new(payload, None);

  assert_eq!(view.downcast_ref::<u32>(), Some(&42_u32));
  assert!(view.sender().is_none());
  assert!(!view.is_control());
  assert!(!view.not_influence_receive_timeout());
  assert!(!view.dead_letter_suppressed());
  assert!(!view.possibly_harmful());
}

#[test]
fn with_control_sets_only_control_flag() {
  let payload: &(dyn Any + Send + Sync + 'static) = &42_u32;
  let sender = ActorRef::null();
  let view = AnyMessageView::with_control(payload, Some(&sender), true);

  assert_eq!(view.downcast_ref::<u32>(), Some(&42_u32));
  assert_eq!(view.sender(), Some(&sender));
  assert!(view.is_control());
  assert!(!view.not_influence_receive_timeout());
  assert!(!view.dead_letter_suppressed());
  assert!(!view.possibly_harmful());
}
