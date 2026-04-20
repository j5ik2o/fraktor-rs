//! AC-H2 ChildrenContainer 4 状態 state machine のテスト。
//!
//! 遷移図 (Pekko 互換仕様):
//! ```text
//! Empty
//!   └─ add_child ─────────────────────────────────────> Normal
//!   └─ shall_die ────────────────────────────────────> Empty (no-op)
//!   └─ set_children_termination_reason ────────────────> false, stays Empty
//!
//! Normal
//!   └─ add_child ─────────────────────────────────────> Normal
//!   └─ shall_die(pid) ───────────────────────────────> Terminating(toDie={pid}, UserRequest)
//!   └─ set_children_termination_reason ────────────────> false, stays Normal
//!   └─ remove_child_and_get_state_change ─────────────> None, stays Normal (or Empty on last)
//!
//! Terminating(toDie, reason)
//!   └─ add_child ─────────────────────────────────────> Terminating (add to c)
//!   └─ shall_die(pid) ───────────────────────────────> Terminating (pid added to toDie)
//!   └─ set_children_termination_reason(r) ─────────────> true, Terminating(reason=r)
//!   └─ remove_child_and_get_state_change(pid):
//!        toDie - pid が空でない → None, stays Terminating
//!        toDie - pid が空 & reason==Termination → Some(Termination), Terminated
//!        toDie - pid が空 & reason!=Termination → Some(reason), Normal (or Empty)
//!
//! Terminated
//!   └─ add_child ─────────────────────────────────────> Terminated (no-op)
//!   └─ shall_die ────────────────────────────────────> Terminated (no-op)
//!   └─ set_children_termination_reason ────────────────> false, stays Terminated
//!   └─ remove_child_and_get_state_change ─────────────> None, stays Terminated
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
  let container = ChildrenContainer::empty();
  assert!(matches!(container, ChildrenContainer::Empty));
  assert!(container.is_normal());
  assert!(!container.is_terminating());
  assert!(container.children().is_empty());
}

#[test]
fn empty_add_child_transitions_to_normal_with_pid_registered() {
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));

  assert!(matches!(container, ChildrenContainer::Normal { .. }));
  assert_eq!(container.children(), vec![pid(1)]);
  assert!(container.is_normal());
  assert!(!container.is_terminating());
}

#[test]
fn empty_set_children_termination_reason_returns_false_and_does_not_transition() {
  let mut container = ChildrenContainer::empty();
  let changed = container.set_children_termination_reason(SuspendReason::Termination);

  assert!(!changed);
  assert!(matches!(container, ChildrenContainer::Empty));
}

#[test]
fn empty_remove_child_and_get_state_change_returns_none_and_stays_empty() {
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
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(1));

  assert_eq!(container.children().len(), 1);
}

#[test]
fn normal_set_children_termination_reason_returns_false_and_stays_normal() {
  // Pekko parity: `setChildrenTerminationReason` は Terminating 状態の container
  // でのみ `true` を返す。Normal 状態では false、state は不変。
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));

  let cause = ActorErrorReason::new("restart-cause");
  let changed = container.set_children_termination_reason(SuspendReason::Recreation(cause));

  assert!(!changed, "Normal state は set_reason に反応しない");
  assert!(matches!(container, ChildrenContainer::Normal { .. }));
}

#[test]
fn normal_shall_die_transitions_to_terminating_with_user_request_reason() {
  // Pekko parity (`ChildrenContainer.scala:140`):
  // NormalChildrenContainer.shallDie(actor) = TerminatingChildrenContainer(c, Set(actor),
  // UserRequest)
  let mut container = ChildrenContainer::empty();
  container.add_child(pid(1));
  container.add_child(pid(2));
  container.shall_die(pid(1));

  assert!(matches!(container, ChildrenContainer::Terminating { .. }));
  let children = container.children();
  assert_eq!(children.len(), 2, "既存の children は保持される");
  assert!(children.contains(&pid(1)));
  assert!(children.contains(&pid(2)));
  // Pekko parity: reason==UserRequest → isNormal=true, isTerminating=false
  assert!(container.is_normal());
  assert!(!container.is_terminating());
}

#[test]
fn normal_remove_child_and_get_state_change_returns_none_and_drops_pid() {
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

/// Drives the container into `Terminating(reason)` with `pids` queued in
/// `to_die`. Uses `shall_die` to transition Normal → Terminating(UserRequest)
/// and then `set_children_termination_reason` to upgrade the reason (Pekko
/// parity with `context.stop(child)` + subsequent `fault_recreate`).
fn into_terminating(reason: SuspendReason, pids: &[Pid]) -> ChildrenContainer {
  let mut container = ChildrenContainer::empty();
  for pid in pids {
    container.add_child(*pid);
  }
  for pid in pids {
    container.shall_die(*pid);
  }
  if !matches!(reason, SuspendReason::UserRequest) {
    let changed = container.set_children_termination_reason(reason);
    assert!(changed, "Terminating 状態の container は set_reason に true を返す");
  }
  container
}

#[test]
fn terminating_set_children_termination_reason_returns_true_and_replaces_reason() {
  let mut container = into_terminating(SuspendReason::Recreation(ActorErrorReason::new("first")), &[pid(1)]);

  let changed = container.set_children_termination_reason(SuspendReason::Termination);

  assert!(changed);
  assert!(container.is_terminating());
  assert!(!container.is_normal());
}

#[test]
fn terminating_variant_reports_is_in_terminating_variant_true_regardless_of_reason() {
  // `is_in_terminating_variant` は Terminating variant を reason 非依存で観測する
  // fraktor-rs 独自ヘルパー。`is_terminating` は Pekko parity のため
  // `reason == Termination` のときのみ true。
  let container = into_terminating(SuspendReason::Recreation(ActorErrorReason::new("boom")), &[pid(1)]);

  assert!(container.is_in_terminating_variant(), "Terminating(Recreation) は is_in_terminating_variant で true");
  assert!(!container.is_terminating(), "Terminating(Recreation) は Pekko parity で is_terminating=false");
  assert!(!container.is_normal(), "Terminating(Recreation) は is_normal=false");
}

#[test]
fn terminating_with_termination_reason_reports_is_terminating_true() {
  // Pekko parity: reason==Termination は is_terminating=true。
  let container = into_terminating(SuspendReason::Termination, &[pid(1)]);
  assert!(container.is_terminating());
  assert!(container.is_in_terminating_variant());
  assert!(!container.is_normal());
}

#[test]
fn terminating_remove_non_last_to_die_returns_none_and_stays_terminating() {
  let mut container = into_terminating(SuspendReason::Termination, &[pid(1), pid(2)]);

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, None);
  assert!(matches!(container, ChildrenContainer::Terminating { .. }));
  assert!(container.is_terminating());
}

#[test]
fn terminating_with_termination_reason_transitions_to_terminated_when_to_die_empties() {
  let mut container = into_terminating(SuspendReason::Termination, &[pid(1)]);

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::Termination));
  assert!(matches!(container, ChildrenContainer::Terminated));
}

#[test]
fn terminating_with_recreation_returns_reason_and_transitions_to_normal() {
  // AC-H4: `set_children_termination_reason(Recreation)` は Normal[c1, c2] を
  // Terminating(to_die=[c1, c2]) に一括遷移させる。したがって最後の子 (c2) の
  // remove で初めて reason が返り、Normal/Empty に戻る。
  let cause = ActorErrorReason::new("boom");
  let mut container = into_terminating(SuspendReason::Recreation(cause.clone()), &[pid(1), pid(2)]);

  let first = container.remove_child_and_get_state_change(pid(1));
  assert_eq!(first, None, "to_die=[c2] が残るため None");
  assert!(matches!(container, ChildrenContainer::Terminating { .. }));

  let second = container.remove_child_and_get_state_change(pid(2));
  assert_eq!(second, Some(SuspendReason::Recreation(cause)));
  assert!(matches!(container, ChildrenContainer::Empty));
}

#[test]
fn terminating_with_recreation_transitions_to_empty_when_last_child_dies() {
  let cause = ActorErrorReason::new("boom");
  let mut container = into_terminating(SuspendReason::Recreation(cause.clone()), &[pid(1)]);

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::Recreation(cause)));
  assert!(matches!(container, ChildrenContainer::Empty));
}

// -----------------------------------------------------------------------------
// Terminated state
// -----------------------------------------------------------------------------

/// Drives the container to the `Terminated` state deterministically.
fn into_terminated_with_single_child() -> ChildrenContainer {
  let mut container = into_terminating(SuspendReason::Termination, &[pid(1)]);
  let _ = container.remove_child_and_get_state_change(pid(1));
  assert!(matches!(container, ChildrenContainer::Terminated));
  container
}

#[test]
fn terminated_reports_terminating_true_and_normal_false() {
  let container = into_terminated_with_single_child();

  assert!(container.is_terminating());
  assert!(!container.is_normal());
}

#[test]
fn terminated_add_child_is_noop() {
  let mut container = into_terminated_with_single_child();

  container.add_child(pid(2));

  assert!(matches!(container, ChildrenContainer::Terminated));
  assert!(container.children().is_empty());
}

#[test]
fn terminated_set_children_termination_reason_returns_false_and_stays_terminated() {
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
  // setChildrenTerminationReason が複数回呼ばれると最後の reason が保持され、
  // 完了時の state_change でそれが返される (Terminating → Terminating で
  // reason のみ更新される)。
  let cause = ActorErrorReason::new("boom");
  let mut container = into_terminating(SuspendReason::Recreation(cause), &[pid(1)]);
  assert!(container.set_children_termination_reason(SuspendReason::Termination));

  let result = container.remove_child_and_get_state_change(pid(1));

  assert_eq!(result, Some(SuspendReason::Termination));
  assert!(matches!(container, ChildrenContainer::Terminated));
}
