use crate::core::kernel::{
  actor::actor_ref::ActorRef,
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
