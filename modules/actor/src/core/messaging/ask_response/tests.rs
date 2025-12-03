use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  actor_prim::actor_ref::ActorRef,
  futures::ActorFuture,
  messaging::{AnyMessage, ask_response::AskResponse},
};

#[test]
fn exposes_parts() {
  let reply: ActorRef = ActorRef::null();
  let future = ActorFuture::<AnyMessage, NoStdToolbox>::new_shared();
  let response = AskResponse::new(reply.clone(), future.clone());

  assert_eq!(response.reply_to(), &reply);
  assert!(!response.future().lock().is_ready());

  future.lock().complete(AnyMessage::new(5_u32));
  assert!(response.future().lock().is_ready());

  let (reply_out, future_out) = response.into_parts();
  assert_eq!(reply_out, reply);
  assert!(future_out.lock().is_ready());
}
