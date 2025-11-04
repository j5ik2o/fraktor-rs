use alloc::string::ToString;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox,
  actor_prim::actor_ref::{actor_ref_sender::ActorRefSender, ask_reply_sender::AskReplySender},
  futures::ActorFuture,
  messaging::{AnyMessage, AnyMessageGeneric},
};

#[test]
fn completes_future_on_send() {
  let future = ArcShared::new(ActorFuture::<AnyMessage, NoStdToolbox>::new());
  let sender: AskReplySender<NoStdToolbox> = AskReplySender::new(future.clone());
  sender.send(AnyMessageGeneric::new("ok".to_string())).unwrap();
  assert!(future.is_ready());
}
