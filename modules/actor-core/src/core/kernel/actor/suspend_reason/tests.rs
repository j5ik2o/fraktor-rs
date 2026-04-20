//! AC-H2 `SuspendReason` の単体テスト。
//!
//! Pekko `ChildrenContainer.scala:55-77` の `SuspendReason` / `WaitingForChildren`
//! ミックスインを翻訳したもの。`Recreation(cause)` と `Creation` は
//! `WaitingForChildren` を持ち、「子の終了完了を待っている最中である」ことを示す。
//! `UserRequest` / `Termination` は `WaitingForChildren` を持たない。

use super::SuspendReason;
use crate::core::kernel::actor::error::ActorErrorReason;

#[test]
fn user_request_is_not_waiting_for_children() {
  // Pekko parity (`ChildrenContainer.scala:63`): `UserRequest` does NOT mix in `WaitingForChildren`.
  let reason = SuspendReason::UserRequest;
  assert!(!reason.is_waiting_for_children());
}

#[test]
fn recreation_is_waiting_for_children() {
  // Pekko parity (`ChildrenContainer.scala:65`):
  // `Recreation(cause)` mixes in `WaitingForChildren`.
  let reason = SuspendReason::Recreation(ActorErrorReason::new("boom"));
  assert!(reason.is_waiting_for_children());
}

#[test]
fn creation_is_waiting_for_children() {
  // Pekko parity (`ChildrenContainer.scala:67`):
  // `Creation` (case object) mixes in `WaitingForChildren`.
  let reason = SuspendReason::Creation;
  assert!(reason.is_waiting_for_children());
}

#[test]
fn termination_is_not_waiting_for_children() {
  // Pekko parity (`ChildrenContainer.scala:69`): `Termination` does NOT mix in `WaitingForChildren`.
  let reason = SuspendReason::Termination;
  assert!(!reason.is_waiting_for_children());
}

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
  assert_eq!(SuspendReason::UserRequest, SuspendReason::UserRequest);
  assert_eq!(SuspendReason::Creation, SuspendReason::Creation);
  assert_eq!(SuspendReason::Termination, SuspendReason::Termination);
  assert_ne!(SuspendReason::UserRequest, SuspendReason::Termination);

  let cause = ActorErrorReason::new("boom");
  assert_eq!(
    SuspendReason::Recreation(cause.clone()),
    SuspendReason::Recreation(cause.clone())
  );
  assert_ne!(
    SuspendReason::Recreation(cause),
    SuspendReason::Recreation(ActorErrorReason::new("other"))
  );
}
