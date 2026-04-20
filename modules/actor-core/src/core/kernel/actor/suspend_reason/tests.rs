//! AC-H2 `SuspendReason` の単体テスト。
//!
//! Pekko `ChildrenContainer.scala:55-77` の `SuspendReason` を翻訳したもの。
//! fraktor-rs の現行定義は `UserRequest` / `Recreation` / `Termination` の 3 variant
//! だが、本テストは AC-H4 restart 経路で中心となる `Recreation` / `Termination` の
//! 等価性契約と cause 保持のみを検証対象とする (`UserRequest` は `shall_die` 経路で
//! 別途カバー)。

use super::SuspendReason;
use crate::core::kernel::actor::error::ActorErrorReason;

#[test]
fn recreation_preserves_cause_reason() {
  // fraktor-rs では Pekko の `Throwable` を `ActorErrorReason` に翻訳しているため、
  // variant が保持する cause がそのまま参照可能であることを確認する。
  let cause = ActorErrorReason::new("restart-reason");
  let reason = SuspendReason::Recreation(cause.clone());

  match reason {
    | SuspendReason::Recreation(inner) => assert_eq!(inner, cause),
    | _ => panic!("expected Recreation variant"),
  }
}

#[test]
fn equality_is_reason_based() {
  // SuspendReason は PartialEq/Eq を derive する想定。`remove_child_and_get_state_change`
  // が返す `Option<SuspendReason>` をテスト側で比較するために必要。
  assert_eq!(SuspendReason::Termination, SuspendReason::Termination);

  let cause = ActorErrorReason::new("boom");
  assert_eq!(SuspendReason::Recreation(cause.clone()), SuspendReason::Recreation(cause.clone()));
  assert_ne!(SuspendReason::Recreation(cause), SuspendReason::Recreation(ActorErrorReason::new("other")));
  assert_ne!(SuspendReason::Recreation(ActorErrorReason::new("x")), SuspendReason::Termination);
}
