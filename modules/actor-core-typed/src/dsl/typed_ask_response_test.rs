use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_ref::ActorRef,
    messaging::{AnyMessage, AskError, AskResponse},
  },
  support::futures::{ActorFuture, ActorFutureShared},
};

use super::TypedAskResponse;

// テスト用ヘルパー: 完了済みの ActorFutureShared<AskResult> を構築する
fn make_completed_future(value: u32) -> ActorFutureShared<Result<AnyMessage, AskError>> {
  let message = AnyMessage::new(value);
  let mut future: ActorFuture<Result<AnyMessage, AskError>> = ActorFuture::new();
  future.complete(Ok(message));
  ActorFutureShared::new(future)
}

// テスト用ヘルパー: AskResponse を構築する（null 送信者 + 完了済み future）
fn make_ask_response(value: u32) -> AskResponse {
  // ActorRef::null() は NullSender を使った公開 API
  let sender = ActorRef::null();
  let future = make_completed_future(value);
  AskResponse::new(sender, future)
}

#[test]
fn from_untyped_constructs_typed_ask_response_from_ask_response() {
  // from_untyped で構築した TypedAskResponse が
  // 既存の取り出し契約（try_take / TypedAskError）で動作すること
  let ask_response = make_ask_response(42_u32);
  let typed: TypedAskResponse<u32> = TypedAskResponse::from_untyped(ask_response);
  let mut future = typed.future().clone();
  assert!(future.is_ready());
  let result = future.try_take().expect("should be ready").expect("should be ok");
  assert_eq!(result, 42_u32);
}

#[test]
fn from_untyped_typed_ask_response_exposes_sender() {
  // from_untyped で構築した TypedAskResponse が sender を参照できること
  let ask_response = make_ask_response(1_u32);
  let typed: TypedAskResponse<u32> = TypedAskResponse::from_untyped(ask_response);
  // sender() は TypedActorRef<u32> を返す（コンパイルが通れば十分）
  let _sender = typed.sender();
}
