//! SP-H1.5: std adaptor 層における panic → `ActorError::Escalate` 変換テスト。
//!
//! 仕様参照:
//! - `.takt/runs/20260419-225627-pekko-kernel-typed-phase-a2/reports/00-plan.md` §94-98
//! - Pekko dispatcher の `try/catch` (panic-to-error 変換)
//!
//! 範囲:
//! - `PanicInvokeGuard` は `actor.receive()` 一文のみを `std::panic::catch_unwind`
//!   で包囲し、panic を `ActorError::Escalate(reason)` へ変換する
//! - lifecycle hooks (`pre_start` / `post_stop` / `pre_restart` / `post_restart`)
//!   は範囲外 (Phase A3 で別途検討)
//! - `NoopInvokeGuard` (no_std default) は素通しで panic 変換しない

use fraktor_actor_adaptor_std_rs::std::actor::PanicInvokeGuard;
use fraktor_actor_core_rs::core::kernel::actor::{
  error::{ActorError, ActorErrorReason},
  invoke_guard::{InvokeGuard, NoopInvokeGuard},
};

#[test]
fn sp_h1_5_t1_panic_in_receive_converts_to_escalate_actor_error() {
  // Given: PanicInvokeGuard を構成
  let guard = PanicInvokeGuard::new();

  // When: panic する receive クロージャを wrap で包む
  let result = guard.wrap(|| -> Result<(), ActorError> { panic!("boom") });

  // Then: panic は捕捉され ActorError::Escalate に変換される
  match result {
    | Err(ActorError::Escalate(_)) => {},
    | other => panic!("expected ActorError::Escalate, got {:?}", other),
  }
}

#[test]
fn sp_h1_5_t2_panic_message_preserved_in_actor_error_reason() {
  // Given: 識別可能な panic メッセージを発生させる guard
  let guard = PanicInvokeGuard::new();

  // When: 特定文字列付きで panic
  let result = guard.wrap(|| -> Result<(), ActorError> { panic!("custom panic detail xyz") });

  // Then: ActorErrorReason の文字列にパニックメッセージが残る
  let Err(ActorError::Escalate(reason)) = result else {
    panic!("expected ActorError::Escalate, got {:?}", result);
  };
  assert!(
    reason.as_str().contains("custom panic detail xyz"),
    "panic message lost: reason = {:?}",
    reason.as_str()
  );
}

#[test]
fn sp_h1_5_t3_normal_receive_passes_through_panic_guard() {
  // Given: 正常な receive クロージャ
  let guard = PanicInvokeGuard::new();

  // When: panic しないクロージャを wrap で包む
  let ok_result = guard.wrap(|| -> Result<(), ActorError> { Ok(()) });

  // Then: Ok はそのまま返る
  assert_eq!(ok_result, Ok(()));

  // And: ActorError::Recoverable / Fatal もそのまま返る (誤って Escalate に変換しない)
  let recoverable = guard.wrap(|| -> Result<(), ActorError> { Err(ActorError::recoverable("planned")) });
  assert_eq!(recoverable, Err(ActorError::Recoverable(ActorErrorReason::new("planned"))));

  let fatal = guard.wrap(|| -> Result<(), ActorError> { Err(ActorError::fatal("planned-fatal")) });
  assert_eq!(fatal, Err(ActorError::Fatal(ActorErrorReason::new("planned-fatal"))));
}

#[test]
fn sp_h1_5_t4_noop_invoke_guard_does_not_catch_panics() {
  // Given: no_std default の素通し guard
  let guard = NoopInvokeGuard::new();

  // When/Then: 通常のクロージャはそのまま結果を返す
  assert_eq!(guard.wrap(|| -> Result<(), ActorError> { Ok(()) }), Ok(()));

  // And: panic は捕捉されず std::panic::catch_unwind で外側から確認できる
  // (PanicInvokeGuard との対比: NoopInvokeGuard は変換責務を持たない)
  let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let _ = guard.wrap(|| -> Result<(), ActorError> { panic!("uncaught by noop guard") });
  }));
  assert!(panicked.is_err(), "NoopInvokeGuard が panic を捕捉してしまった");
}
