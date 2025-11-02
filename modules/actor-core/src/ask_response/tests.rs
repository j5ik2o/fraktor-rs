#![cfg(test)]

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{actor_future::ActorFuture, any_message::AnyMessage, ask_response::AskResponse, ActorRef};

#[test]
fn exposes_parts() {
  let reply = ActorRef::null();
  let future = ArcShared::new(ActorFuture::new());
  let response = AskResponse::new(reply.clone(), future.clone());

  assert_eq!(response.reply_to(), &reply);
  assert!(core::ptr::eq(response.future(), &future));

  let (reply_out, future_out) = response.into_parts();
  assert_eq!(reply_out, reply);
  assert!(core::ptr::eq(&future_out, &future));
}
