use alloc::string::ToString;

use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  actor_prim::actor_ref::{actor_ref_sender::ActorRefSender, ask_reply_sender::AskReplySender},
  futures::ActorFuture,
  messaging::AnyMessage,
};

#[test]
fn completes_future_on_send() {
  let future = ArcShared::new(ActorFuture::<AnyMessage, NoStdToolbox>::new());
  let sender: AskReplySender = AskReplySender::new(future.clone());
  sender.send(AnyMessage::new("ok".to_string())).unwrap();
  assert!(future.is_ready());
}
