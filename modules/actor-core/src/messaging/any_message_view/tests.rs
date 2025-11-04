use crate::{
  actor_prim::actor_ref::{ActorRef, ActorRefGeneric},
  messaging::{AnyMessage, AnyMessageGeneric, AnyMessageView},
};

#[test]
fn downcasts_payload() {
  let message: AnyMessage = AnyMessageGeneric::new(10_i32);
  let view: AnyMessageView<'_, _> = message.as_view();
  assert_eq!(view.downcast_ref::<i32>(), Some(&10));
}

#[test]
fn carries_reply_reference() {
  let reply: ActorRef = ActorRefGeneric::null();
  let message = AnyMessageGeneric::new("ping").with_reply_to(reply.clone());
  let view = message.as_view();
  assert!(matches!(view.reply_to(), Some(r) if r == &reply));
}
