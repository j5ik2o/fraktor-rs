#![cfg(test)]

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{ActorRef, NoStdToolbox, actor_future::ActorFuture, any_message::AnyMessage, ask_response::AskResponse};

#[test]
fn exposes_parts() {
  let reply: ActorRef<NoStdToolbox> = ActorRef::null();
  let future = ArcShared::new(ActorFuture::<AnyMessage<NoStdToolbox>, NoStdToolbox>::new());
  let response = AskResponse::new(reply.clone(), future.clone());

  assert_eq!(response.reply_to(), &reply);
  assert!(!response.future().is_ready());

  future.complete(AnyMessage::new(5_u32));
  assert!(response.future().is_ready());

  let (reply_out, future_out) = response.into_parts();
  assert_eq!(reply_out, reply);
  assert!(future_out.is_ready());
}
