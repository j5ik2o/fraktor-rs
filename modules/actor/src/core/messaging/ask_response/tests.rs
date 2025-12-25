use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::SharedAccess};

use crate::core::{
  actor_prim::actor_ref::ActorRef,
  futures::ActorFutureSharedGeneric,
  messaging::{AnyMessage, ask_response::AskResponse},
};

#[test]
fn exposes_parts() {
  let sender: ActorRef = ActorRef::null();
  let future = ActorFutureSharedGeneric::<AnyMessage, NoStdToolbox>::new();
  let response = AskResponse::new(sender.clone(), future.clone());

  assert_eq!(response.sender(), &sender);
  assert!(!response.future().with_write(|af| af.is_ready()));

  future.with_write(|af| af.complete(AnyMessage::new(5_u32)));
  assert!(response.future().with_read(|af| af.is_ready()));

  let (sender_out, future_out) = response.into_parts();
  assert_eq!(sender_out, sender);
  assert!(future_out.with_read(|af| af.is_ready()));
}
