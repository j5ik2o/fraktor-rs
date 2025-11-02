#![cfg(test)]

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{actor_future::ActorFuture, actor_ref::ask_reply_sender::AskReplySender, any_message::AnyMessage};

#[test]
fn completes_future_on_send() {
  let future = ArcShared::new(ActorFuture::new());
  let sender = AskReplySender::new(future.clone());
  sender.send(AnyMessage::new("ok")).unwrap();
  assert!(future.is_ready());
}
