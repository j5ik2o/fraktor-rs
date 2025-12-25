use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::SharedAccess};

use crate::core::{
  actor_prim::actor_ref::ActorRef,
  futures::ActorFutureSharedGeneric,
  messaging::{AnyMessage, AskError, AskResult, ask_response::AskResponse},
};

type TestAskResult = AskResult<NoStdToolbox>;

#[test]
fn exposes_parts() {
  let sender: ActorRef = ActorRef::null();
  let future = ActorFutureSharedGeneric::<TestAskResult, NoStdToolbox>::new();
  let response = AskResponse::new(sender.clone(), future.clone());

  assert_eq!(response.sender(), &sender);
  assert!(!response.future().with_write(|af| af.is_ready()));

  future.with_write(|af| af.complete(Ok(AnyMessage::new(5_u32))));
  assert!(response.future().with_read(|af| af.is_ready()));

  let (sender_out, future_out) = response.into_parts();
  assert_eq!(sender_out, sender);
  assert!(future_out.with_read(|af| af.is_ready()));
}

#[test]
fn future_resolves_with_success() {
  let sender: ActorRef = ActorRef::null();
  let future = ActorFutureSharedGeneric::<TestAskResult, NoStdToolbox>::new();
  let response = AskResponse::new(sender, future.clone());

  // 成功値で完了
  future.with_write(|af| af.complete(Ok(AnyMessage::new(42_u32))));

  // future から結果を取り出す
  let result = response.future().with_write(|af| af.try_take());
  assert!(result.is_some());
  let ask_result = result.unwrap();
  assert!(ask_result.is_ok());
}

#[test]
fn future_resolves_with_timeout_error() {
  let sender: ActorRef = ActorRef::null();
  let future = ActorFutureSharedGeneric::<TestAskResult, NoStdToolbox>::new();
  let response = AskResponse::new(sender, future.clone());

  // タイムアウトエラーで完了
  future.with_write(|af| af.complete(Err(AskError::Timeout)));

  let result = response.future().with_write(|af| af.try_take());
  assert!(result.is_some());
  let ask_result = result.unwrap();
  assert!(ask_result.is_err());
  assert_eq!(ask_result.unwrap_err(), AskError::Timeout);
}

#[test]
fn future_resolves_with_dead_letter_error() {
  let sender: ActorRef = ActorRef::null();
  let future = ActorFutureSharedGeneric::<TestAskResult, NoStdToolbox>::new();
  let response = AskResponse::new(sender, future.clone());

  // DeadLetter エラーで完了
  future.with_write(|af| af.complete(Err(AskError::DeadLetter)));

  let result = response.future().with_write(|af| af.try_take());
  assert!(result.is_some());
  let ask_result = result.unwrap();
  assert!(ask_result.is_err());
  assert_eq!(ask_result.unwrap_err(), AskError::DeadLetter);
}

#[test]
fn future_resolves_with_send_failed_error() {
  let sender: ActorRef = ActorRef::null();
  let future = ActorFutureSharedGeneric::<TestAskResult, NoStdToolbox>::new();
  let response = AskResponse::new(sender, future.clone());

  // SendFailed エラーで完了
  future.with_write(|af| af.complete(Err(AskError::SendFailed)));

  let result = response.future().with_write(|af| af.try_take());
  assert!(result.is_some());
  let ask_result = result.unwrap();
  assert!(ask_result.is_err());
  assert_eq!(ask_result.unwrap_err(), AskError::SendFailed);
}
