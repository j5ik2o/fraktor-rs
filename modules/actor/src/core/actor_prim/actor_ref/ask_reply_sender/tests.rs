use alloc::string::ToString;

use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::SharedAccess};

use crate::core::{
  actor_prim::actor_ref::{actor_ref_sender::ActorRefSender, ask_reply_sender::AskReplySender},
  futures::ActorFutureSharedGeneric,
  messaging::AnyMessage,
};

#[test]
fn completes_future_on_send() {
  let future = ActorFutureSharedGeneric::<AnyMessage, NoStdToolbox>::new();
  let mut sender: AskReplySender = AskReplySender::new(future.clone());
  sender.send(AnyMessage::new("ok".to_string())).unwrap();
  assert!(future.with_write(|af| af.is_ready()));
}
