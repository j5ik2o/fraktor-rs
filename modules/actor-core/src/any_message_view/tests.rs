#![cfg(test)]

use crate::{any_message::AnyMessage, any_message_view::AnyMessageView, ActorRef, NoStdToolbox};

#[test]
fn downcasts_payload() {
  let message: AnyMessage<NoStdToolbox> = AnyMessage::new(10_i32);
  let view: AnyMessageView<'_, _> = message.as_view();
  assert_eq!(view.downcast_ref::<i32>(), Some(&10));
}

#[test]
fn carries_reply_reference() {
  let reply: ActorRef<NoStdToolbox> = ActorRef::null();
  let message = AnyMessage::new("ping").with_reply_to(reply.clone());
  let view = message.as_view();
  assert!(matches!(view.reply_to(), Some(r) if r == &reply));
}
