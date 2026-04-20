//! AC-H2 ChildrenContainer 4 状態 state machine のテスト。
//!
//! Pekko `ChildrenContainer`（`references/pekko/.../dungeon/ChildrenContainer.scala`）と
//! `setChildrenTerminationReason` / `removeChildAndGetStateChange`
//! （`dungeon/Children.scala:178-257`）のセマンティクスを忠実に翻訳していることを検証する。
//!
//! 遷移図（Pekko 仕様）:
//! ```text
//! Empty
//!   └─ add_child ─────────────────────────────> Normal
//!   └─ shall_die ────────────────────────────> Empty (no-op)
//!   └─ set_children_termination_reason ──────> false, Empty
//!
//! Normal
//!   └─ add_child ────────────────────────────> Normal
//!   └─ shall_die(pid) ───────────────────────> Terminating(toDie={pid}, UserRequest)
//!   └─ set_children_termination_reason ──────> false, Normal
//!   └─ remove_child_and_get_state_change ────> None, Normal (or Empty if last)
//!
//! Terminating(toDie, reason)
//!   └─ add_child ────────────────────────────> Terminating (add to c)
//!   └─ shall_die(pid) ───────────────────────> Terminating (pid added to toDie)
//!   └─ set_children_termination_reason(r) ───> true, Terminating(reason=r)
//!   └─ remove_child_and_get_state_change(pid):
//!        toDie - pid が空でない → None, stays Terminating
//!        toDie - pid が空 & reason==Termination → Some(Termination), Terminated
//!        toDie - pid が空 & reason!=Termination → Some(reason), Normal (or Empty)
//!
//! Terminated
//!   └─ add_child ────────────────────────────> Terminated (no-op)
//!   └─ shall_die ────────────────────────────> Terminated (no-op)
//!   └─ set_children_termination_reason ──────> false, Terminated
//!   └─ remove_child_and_get_state_change ────> None, Terminated
//! ```

use alloc::vec;

use super::ChildrenContainer;
use crate::core::kernel::actor::{Pid, error::ActorErrorReason, suspend_reason::SuspendReason};

/// Returns a deterministic `Pid` for tests.
fn pid(value: u64) -> Pid {
  Pid::new(value, 0)
}

// -----------------------------------------------------------------------------
// Empty state
// -----------------------------------------------------------------------------

#[test]
fn empty_reports_normal_and_holds_no_children() {
  // Pekko parity: `EmptyChildrenContainer` extends the default `isNormal=true`,
  // `isTerminating=false` and exposes no children.
  let container = ChildrenContainer::empty();
  assert!(matches!(container, ChildrenContainer::Empty));
  assert!(container.is_normal());
  assert!(!container.is_terminating());
  assert!(container.children().is_empty());
}

#[test]
fn empty_add_child_transitions_to_normal_with_pid_registered() {
  // Pekko parity: EmptyChildrenContainer.add returns NormalChildrenContainer(updated c).
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));

  assert!(matches!(container, ChildrenContainer::Normal { .. }));
  assert_eq!(container.children(), vec![pid(1)]);
  assert!(container.is_normal());
  assert!(!container.is_terminating());
}

#[test]
fn empty_shall_die_is_noop() {
  // Pekko parity: EmptyChildrenContainer.shallDie returns `this`.
  let mut container = ChildrenContainer::empty();
  container.shall_die(pid(1));

  assert!(matches!(container, ChildrenContainer::Empty));
  assert!(container.children().is_empty());
}

#[test]
fn empty_set_children_termination_reason_returns_false_and_does_not_transition() {
  // Pekko parity (`Children.scala:178-183`): setChildrenTerminationReason returns false
  // whenever the current state is not TerminatingChildrenContainer.
  let mut container = ChildrenContainer::empty();
  let changed = container.set_children_termination_reason(SuspendReason::Termination);

  assert!(!changed);
  assert!(matches!(container, ChildrenContainer::Empty));
}

#[test]
fn empty_remove_child_and_get_state_change_returns_none_and_stays_empty() {
  // Pekko parity (`Children.scala:247-255`): non-Terminating states always return None.
  let mut container = ChildrenContainer::empty();
  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, None);
  assert!(matches!(container, ChildrenContainer::Empty));
}

// -----------------------------------------------------------------------------
// Normal state
// -----------------------------------------------------------------------------

#[test]
fn normal_add_multiple_children_preserves_all() {
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(2));
  container.add_child(pid(3));

  let children = container.children();
  assert_eq!(children.len(), 3);
  assert!(children.contains(&pid(1)));
  assert!(children.contains(&pid(2)));
  assert!(children.contains(&pid(3)));
  assert!(matches!(container, ChildrenContainer::Normal { .. }));
}

#[test]
fn normal_add_child_is_idempotent_for_same_pid() {
  // Pekko map semantics: `c.updated(name, stats)` で既存キーは更新され、重複しない。
  // fraktor-rs は Pid ベースで同等にデデュプ。
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(1));

  assert_eq!(container.children().len(), 1);
}

#[test]
fn normal_shall_die_transitions_to_terminating_with_user_request_reason() {
  // Pekko parity (`ChildrenContainer.scala:140`):
  // NormalChildrenContainer.shallDie(actor) = TerminatingChildrenContainer(c, Set(actor), UserRequest)
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(2));
  container.shall_die(pid(1));

  // Underlying state is now Terminating, but the full child set (c) is preserved.
  assert!(matches!(container, ChildrenContainer::Terminating { .. }));
  let children = container.children();
  assert_eq!(children.len(), 2);
  assert!(children.contains(&pid(1)));
  assert!(children.contains(&pid(2)));
  // Pekko parity: isNormal=true when reason==UserRequest, isTerminating=false.
  assert!(container.is_normal());
  assert!(!container.is_terminating());
}

#[test]
fn normal_set_children_termination_reason_returns_false_and_stays_normal() {
  // Pekko parity: setChildrenTerminationReason only updates existing Terminating states.
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));

  let changed = container.set_children_termination_reason(SuspendReason::Termination);

  assert!(!changed);
  assert!(matches!(container, ChildrenContainer::Normal { .. }));
}

#[test]
fn normal_remove_child_and_get_state_change_returns_none_and_drops_pid() {
  // Pekko parity (`Children.scala:253-255`): Normal の remove_child は None を返し、
  // 内部的に pid を除去。
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(2));

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, None);
  assert_eq!(container.children(), vec![pid(2)]);
  assert!(matches!(container, ChildrenContainer::Normal { .. }));
}

#[test]
fn normal_remove_last_child_falls_back_to_empty() {
  // Pekko parity (`ChildrenContainer.scala:157-160`):
  // NormalChildrenContainer.apply(c) with c.isEmpty returns EmptyChildrenContainer.
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, None);
  assert!(matches!(container, ChildrenContainer::Empty));
}

#[test]
fn normal_stats_for_returns_restart_statistics_for_registered_pid() {
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));

  assert!(container.stats_for(pid(1)).is_some());
}

#[test]
fn normal_stats_for_returns_none_for_unknown_pid() {
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));

  assert!(container.stats_for(pid(999)).is_none());
}

// -----------------------------------------------------------------------------
// Terminating state
// -----------------------------------------------------------------------------

#[test]
fn terminating_set_children_termination_reason_returns_true_and_replaces_reason() {
  // Pekko parity (`Children.scala:180-181`): `TerminatingChildrenContainer.copy(reason=newReason)`.
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));

  let changed = container.set_children_termination_reason(SuspendReason::Termination);

  assert!(changed);
  // After reason replacement, is_terminating should reflect the new reason (Termination).
  assert!(container.is_terminating());
  assert!(!container.is_normal());
}

#[test]
fn terminating_with_user_request_is_normal_and_not_terminating() {
  // Pekko parity (`ChildrenContainer.scala:218-219`):
  //   isTerminating = reason == Termination
  //   isNormal      = reason == UserRequest
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));

  assert!(matches!(container, ChildrenContainer::Terminating { .. }));
  assert!(container.is_normal());
  assert!(!container.is_terminating());
}

#[test]
fn terminating_with_recreation_is_neither_normal_nor_terminating() {
  let cause = ActorErrorReason::new("boom");
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));
  let changed = container.set_children_termination_reason(SuspendReason::Recreation(cause));
  assert!(changed);

  // Pekko: reason==Recreation → isNormal=false, isTerminating=false
  assert!(!container.is_normal());
  assert!(!container.is_terminating());
}

#[test]
fn terminating_with_creation_is_neither_normal_nor_terminating() {
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));
  let changed = container.set_children_termination_reason(SuspendReason::Creation);
  assert!(changed);

  assert!(!container.is_normal());
  assert!(!container.is_terminating());
}

#[test]
fn terminating_shall_die_adds_pid_to_to_die_set() {
  // Pekko parity (`ChildrenContainer.scala:203`):
  // TerminatingChildrenContainer.shallDie(actor) = copy(toDie = toDie + actor)
  // よって初期 toDie={1} + shall_die(2) の後、toDie={1,2} であり、
  // 両方を removeChild するまで Terminated へ遷移しない。
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(2));
  container.shall_die(pid(1));
  container.shall_die(pid(2));
  let changed = container.set_children_termination_reason(SuspendReason::Termination);
  assert!(changed);

  // Remove first: toDie still has {pid(2)}, state stays Terminating.
  let first = container.remove_child_and_get_state_change(pid(1));
  assert_eq!(first, None);
  assert!(matches!(container, ChildrenContainer::Terminating { .. }));

  // Remove second: toDie becomes empty under Termination → transition to Terminated.
  let second = container.remove_child_and_get_state_change(pid(2));
  assert_eq!(second, Some(SuspendReason::Termination));
  assert!(matches!(container, ChildrenContainer::Terminated));
}

#[test]
fn terminating_remove_non_last_to_die_returns_none_and_stays_terminating() {
  // Pekko parity (`ChildrenContainer.scala:181-188`):
  // remove(child) with (toDie - child) non-empty returns copy(c - name, t) = Terminating.
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(2));
  container.shall_die(pid(1));
  container.shall_die(pid(2));
  container.set_children_termination_reason(SuspendReason::Termination);

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, None);
  assert!(matches!(container, ChildrenContainer::Terminating { .. }));
  assert!(container.is_terminating());
}

#[test]
fn terminating_with_termination_reason_transitions_to_terminated_when_to_die_empties() {
  // Pekko parity (`ChildrenContainer.scala:181-188`):
  // remove(child) with (toDie - child) empty and reason==Termination → TerminatedChildrenContainer.
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));
  container.set_children_termination_reason(SuspendReason::Termination);

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::Termination));
  assert!(matches!(container, ChildrenContainer::Terminated));
}

#[test]
fn terminating_with_user_request_transitions_to_normal_when_more_children_remain() {
  // Pekko parity (`ChildrenContainer.scala:181-188`):
  // remove(child) with (toDie - child) empty and reason!=Termination → NormalChildrenContainer(c - name).
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(2));
  container.shall_die(pid(1));

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::UserRequest));
  assert!(matches!(container, ChildrenContainer::Normal { .. }));
  assert_eq!(container.children(), vec![pid(2)]);
}

#[test]
fn terminating_with_user_request_transitions_to_empty_when_last_child_dies() {
  // Pekko parity (`ChildrenContainer.scala:157-160`):
  // NormalChildrenContainer(c - name) with c empty falls back to EmptyChildrenContainer.
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::UserRequest));
  assert!(matches!(container, ChildrenContainer::Empty));
}

#[test]
fn terminating_with_recreation_returns_reason_and_transitions_to_normal() {
  // Pekko parity: Recreation is not Termination, so after toDie empties the container
  // falls back to Normal (or Empty) and returns Some(Recreation(cause)).
  let cause = ActorErrorReason::new("boom");
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(2));
  container.shall_die(pid(1));
  container.set_children_termination_reason(SuspendReason::Recreation(cause.clone()));

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::Recreation(cause)));
  assert!(matches!(container, ChildrenContainer::Normal { .. }));
  assert_eq!(container.children(), vec![pid(2)]);
}

#[test]
fn terminating_with_creation_returns_reason_and_transitions_to_empty_when_last_child_dies() {
  // Pekko parity: Creation is not Termination, so after toDie empties with no siblings left
  // the container falls back to Empty and returns Some(Creation).
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));
  container.set_children_termination_reason(SuspendReason::Creation);

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::Creation));
  assert!(matches!(container, ChildrenContainer::Empty));
}

// -----------------------------------------------------------------------------
// Terminated state
// -----------------------------------------------------------------------------

/// Drives the container to the `Terminated` state deterministically.
fn into_terminated_with_single_child() -> ChildrenContainer {
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));
  container.set_children_termination_reason(SuspendReason::Termination);
  let _ = container.remove_child_and_get_state_change(pid(1));
  assert!(matches!(container, ChildrenContainer::Terminated));
  container
}

#[test]
fn terminated_reports_terminating_true_and_normal_false() {
  // Pekko parity (`ChildrenContainer.scala:109-110`):
  //   TerminatedChildrenContainer overrides isTerminating=true, isNormal=false.
  let container = into_terminated_with_single_child();

  assert!(container.is_terminating());
  assert!(!container.is_normal());
}

#[test]
fn terminated_add_child_is_noop() {
  // Pekko parity (`ChildrenContainer.scala:106`):
  // TerminatedChildrenContainer.add returns `this` - no state change, no child registered.
  let mut container = into_terminated_with_single_child();

  container.add_child(pid(2));

  assert!(matches!(container, ChildrenContainer::Terminated));
  assert!(container.children().is_empty());
}

#[test]
fn terminated_shall_die_is_noop() {
  // Pekko parity: TerminatedChildrenContainer inherits EmptyChildrenContainer.shallDie = `this`.
  let mut container = into_terminated_with_single_child();

  container.shall_die(pid(99));

  assert!(matches!(container, ChildrenContainer::Terminated));
}

#[test]
fn terminated_set_children_termination_reason_returns_false_and_stays_terminated() {
  // Pekko parity: setChildrenTerminationReason returns false whenever current state
  // is not TerminatingChildrenContainer; Terminated is not Terminating.
  let mut container = into_terminated_with_single_child();

  let changed = container.set_children_termination_reason(SuspendReason::Termination);

  assert!(!changed);
  assert!(matches!(container, ChildrenContainer::Terminated));
}

#[test]
fn terminated_remove_child_and_get_state_change_returns_none_and_stays_terminated() {
  let mut container = into_terminated_with_single_child();

  let result = container.remove_child_and_get_state_change(pid(99));

  assert_eq!(result, None);
  assert!(matches!(container, ChildrenContainer::Terminated));
}

// -----------------------------------------------------------------------------
// Reason-driven transition matrix
// -----------------------------------------------------------------------------

#[test]
fn reason_override_preserves_most_recent_reason_returned_on_completion() {
  // Pekko parity: setChildrenTerminationReason が複数回呼ばれると最後の reason が保持される。
  // そのため remove_child_and_get_state_change が返す Some(reason) も最後に設定した reason。
  let cause = ActorErrorReason::new("boom");
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.shall_die(pid(1));
  // First: UserRequest → Recreation
  assert!(container.set_children_termination_reason(SuspendReason::Recreation(cause.clone())));
  // Then: Recreation → Termination
  assert!(container.set_children_termination_reason(SuspendReason::Termination));

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::Termination));
  assert!(matches!(container, ChildrenContainer::Terminated));
}
