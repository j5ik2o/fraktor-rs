#![cfg(test)]

use alloc::string::ToString;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  actor_future::ActorFuture,
  actor_ref::{actor_ref_sender::ActorRefSender, ask_reply_sender::AskReplySender},
  any_message::AnyMessage,
  NoStdToolbox,
};

#[test]
fn completes_future_on_send() {
  let future = ArcShared::new(ActorFuture::<AnyMessage<NoStdToolbox>, NoStdToolbox>::new());
  let sender: AskReplySender<NoStdToolbox> = AskReplySender::new(future.clone());
  sender.send(AnyMessage::new("ok".to_string())).unwrap();
  assert!(future.is_ready());
}
