#![cfg(test)]

use crate::{any_message::AnyMessage, pid::Pid, ActorRef, NoStdToolbox};

#[test]
fn stores_payload_and_reply_to() {
  let mut message: AnyMessage<NoStdToolbox> = AnyMessage::new(5_u32);
  assert_eq!(message.payload().downcast_ref::<u32>(), Some(&5));

  let reply: ActorRef<NoStdToolbox> = ActorRef::null();
  message = message.with_reply_to(reply.clone());
  assert_eq!(message.reply_to(), Some(&reply));

  let view = message.as_view();
  assert_eq!(view.downcast_ref::<u32>(), Some(&5));
  assert!(view.reply_to().is_some());
  assert_eq!(view.reply_to().unwrap().pid(), Pid::new(0, 0));
}
