use fraktor_actor_core_kernel_rs::{
  actor::messaging::{AnyMessage, AskError},
  support::futures::{ActorFuture, ActorFutureShared},
};

use super::TypedAskFuture;
use crate::dsl::TypedAskError;

// テスト用ヘルパー: 完了済みの ActorFutureShared<AskResult> を構築する
fn make_completed_future(value: u32) -> ActorFutureShared<Result<AnyMessage, AskError>> {
  let message = AnyMessage::new(value);
  let mut future: ActorFuture<Result<AnyMessage, AskError>> = ActorFuture::new();
  future.complete(Ok(message));
  ActorFutureShared::new(future)
}

// テスト用ヘルパー: 失敗した ActorFutureShared<AskResult> を構築する
fn make_failed_future() -> ActorFutureShared<Result<AnyMessage, AskError>> {
  let mut future: ActorFuture<Result<AnyMessage, AskError>> = ActorFuture::new();
  future.complete(Err(AskError::Timeout));
  ActorFutureShared::new(future)
}

#[test]
fn from_untyped_constructs_typed_ask_future_and_try_take_returns_ok() {
  // from_untyped で構築した TypedAskFuture が
  // 既存の取り出し契約（try_take）で動作すること
  let inner = make_completed_future(99_u32);
  let mut typed: TypedAskFuture<u32> = TypedAskFuture::from_untyped(inner);
  assert!(typed.is_ready());
  let result = typed.try_take().expect("should be ready").expect("should be ok");
  assert_eq!(result, 99_u32);
}

#[test]
fn from_untyped_typed_ask_future_propagates_ask_error() {
  // from_untyped で構築した TypedAskFuture が
  // 失敗時に TypedAskError::AskFailed を返すこと
  let inner = make_failed_future();
  let mut typed: TypedAskFuture<u32> = TypedAskFuture::from_untyped(inner);
  assert!(typed.is_ready());
  let result = typed.try_take().expect("should be ready");
  assert!(matches!(result, Err(TypedAskError::AskFailed(AskError::Timeout))));
}

#[test]
fn from_untyped_typed_ask_future_type_mismatch_returns_error() {
  // 応答型が一致しない場合に TypedAskError::TypeMismatch が返ること
  // u32 として完了させたものを u64 として取り出す
  let inner = make_completed_future(1_u32);
  let mut typed: TypedAskFuture<u64> = TypedAskFuture::from_untyped(inner);
  assert!(typed.is_ready());
  let result = typed.try_take().expect("should be ready");
  assert!(matches!(result, Err(TypedAskError::TypeMismatch)));
}
