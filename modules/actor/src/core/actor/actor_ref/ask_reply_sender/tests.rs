use alloc::string::ToString;

use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::SharedAccess};

use crate::core::{
  actor::actor_ref::{actor_ref_sender::ActorRefSender, ask_reply_sender::AskReplySender},
  futures::ActorFutureSharedGeneric,
  messaging::{AnyMessage, AskResult},
};

type TestAskResult = AskResult<NoStdToolbox>;

#[test]
fn completes_future_on_send() {
  let future = ActorFutureSharedGeneric::<TestAskResult, NoStdToolbox>::new();
  let mut sender: AskReplySender = AskReplySender::new(future.clone());
  sender.send(AnyMessage::new("ok".to_string())).unwrap();
  assert!(future.with_write(|af| af.is_ready()));
}

#[test]
fn reply_is_wrapped_in_ok() {
  let future = ActorFutureSharedGeneric::<TestAskResult, NoStdToolbox>::new();
  let mut sender: AskReplySender = AskReplySender::new(future.clone());
  sender.send(AnyMessage::new(42_u32)).unwrap();

  let result = future.with_write(|af| af.try_take());
  assert!(result.is_some());
  let ask_result = result.unwrap();
  assert!(ask_result.is_ok(), "reply should be wrapped in Ok");
}
