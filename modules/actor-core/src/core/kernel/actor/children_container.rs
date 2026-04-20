//! Four-state child-registry state machine ported from Pekko
//! `ChildrenContainer` (`references/pekko/.../dungeon/ChildrenContainer.scala`).
//!
//! The container tracks the lifecycle of the children supervised by an
//! `ActorCell`. The four states (`Empty`, `Normal`, `Terminating`, `Terminated`)
//! and their transitions mirror Pekko exactly; see the module-level doc-comment
//! of `children_container/tests.rs` for the transition diagram.
//!
//! Notable deviations from Pekko:
//!
//! * Pekko keys children by `String` (the actor name); fraktor-rs keys them by
//!   [`Pid`] because the kernel layer does not expose actor names to the
//!   container.
//! * Pekko stores [`ChildRestartStats`] with additional `uid` / `child` fields;
//!   fraktor-rs stores a plain [`RestartStatistics`] next to the pid because
//!   the uid is already part of [`Pid::generation`] and the child reference is
//!   resolved through the [`SystemStateShared`] registry.
//! * Pekko's `reserve` / `unreserve` APIs are intentionally not ported here;
//!   fraktor-rs reserves names through [`SystemStateShared::reserve_name`]
//!   which is orthogonal to supervision bookkeeping.
//!
//! [`ChildRestartStats`]: https://github.com/apache/pekko/blob/main/actor/src/main/scala/org/apache/pekko/actor/ChildRestartStats.scala
//! [`SystemStateShared::reserve_name`]: crate::core::kernel::system::state::SystemStateShared

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::core::kernel::actor::{Pid, suspend_reason::SuspendReason, supervision::RestartStatistics};

/// Child-registry state machine.
///
/// Each variant corresponds to one of the four Pekko subtypes:
///
/// | Variant         | Pekko counterpart                                   |
/// |-----------------|-----------------------------------------------------|
/// | [`Empty`]       | `EmptyChildrenContainer`                            |
/// | [`Normal`]      | `NormalChildrenContainer`                           |
/// | [`Terminating`] | `TerminatingChildrenContainer(c, toDie, reason)`    |
/// | [`Terminated`]  | `TerminatedChildrenContainer`                       |
///
/// [`Empty`]: ChildrenContainer::Empty
/// [`Normal`]: ChildrenContainer::Normal
/// [`Terminating`]: ChildrenContainer::Terminating
/// [`Terminated`]: ChildrenContainer::Terminated
//
// `Terminating` は AC-H2 で `shall_die` / `set_children_termination_reason`
// 経由の生成経路を実装しているが、production の呼び出し側 (`handle_recreate` /
// `finish_terminate`) は AC-H4 で配線される。現時点では test 経由でのみ
// 到達可能なため `#[allow(dead_code)]` を付与する。plan § バッチ分割で scope
// 明示済み、`children_container/tests.rs` 全 29 ケースで網羅テスト済み。
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum ChildrenContainer {
  /// No children registered. Default state for a freshly spawned actor cell.
  ///
  /// Pekko parity: `EmptyChildrenContainer`
  /// (`ChildrenContainer.scala:78-98`).
  Empty,
  /// At least one child is registered and none of them is currently
  /// terminating.
  ///
  /// Pekko parity: `NormalChildrenContainer(c)`
  /// (`ChildrenContainer.scala:114-161`).
  Normal {
    /// Child registry keyed by pid. Values carry [`RestartStatistics`].
    c: Vec<(Pid, RestartStatistics)>,
  },
  /// At least one child was told to stop and the parent is waiting for the
  /// corresponding `Terminated` system messages.
  ///
  /// Pekko parity: `TerminatingChildrenContainer(c, toDie, reason)`
  /// (`ChildrenContainer.scala:163-224`).
  Terminating {
    /// Child registry keyed by pid. Values carry [`RestartStatistics`].
    c: Vec<(Pid, RestartStatistics)>,
    /// Pids that were asked to die but whose `Terminated` has not yet arrived.
    to_die: Vec<Pid>,
    /// Reason this transition was initiated.
    reason: SuspendReason,
  },
  /// Terminal state installed after the last child has terminated while the
  /// parent itself is stopping.
  ///
  /// Pekko parity: `TerminatedChildrenContainer`
  /// (`ChildrenContainer.scala:100-112`).
  Terminated,
}

impl ChildrenContainer {
  /// Creates a fresh container in the [`Empty`](Self::Empty) state.
  #[must_use]
  pub(crate) const fn empty() -> Self {
    Self::Empty
  }

  /// Registers `pid` as a supervised child.
  ///
  /// * [`Empty`](Self::Empty) transitions to [`Normal`](Self::Normal).
  /// * [`Normal`](Self::Normal) / [`Terminating`](Self::Terminating) update
  ///   their child map in place (idempotent for already-registered pids).
  /// * [`Terminated`](Self::Terminated) is a no-op — Pekko parity
  ///   (`ChildrenContainer.scala:106`: `TerminatedChildrenContainer.add =
  ///   this`).
  pub(crate) fn add_child(&mut self, pid: Pid) {
    match self {
      | Self::Empty => {
        *self = Self::Normal { c: alloc::vec![(pid, RestartStatistics::new())] };
      },
      | Self::Normal { c } | Self::Terminating { c, .. } => {
        insert_child_if_absent(c, pid);
      },
      | Self::Terminated => {
        // Pekko parity (`ChildrenContainer.scala:106`): terminated containers
        // ignore additional registrations.
      },
    }
  }

  /// Marks `pid` as scheduled to die, returning the previous reason unchanged.
  ///
  /// * [`Empty`](Self::Empty) / [`Terminated`](Self::Terminated) are no-ops —
  ///   Pekko parity (`ChildrenContainer.scala:87`: `shallDie = this`).
  /// * [`Normal`](Self::Normal) transitions to
  ///   [`Terminating`](Self::Terminating) with `reason = UserRequest`.
  /// * [`Terminating`](Self::Terminating) adds `pid` to the `to_die` set
  ///   without touching `reason`.
  //
  // AC-H4 で `handle_recreate` / `terminate` 経路 (pekko `Children.scala:210`)
  // と連携して配線予定。AC-H3 は `suspend_children` / `resume_children` による
  // 子 mailbox 再帰のみを対象とし `shall_die` を使わない。現時点では test
  // 経由でのみ到達するため `#[allow(dead_code)]` を付与する。
  #[allow(dead_code)]
  pub(crate) fn shall_die(&mut self, pid: Pid) {
    match self {
      | Self::Empty | Self::Terminated => {
        // Pekko parity: no-op for Empty and Terminated.
      },
      | Self::Normal { c } => {
        // Pekko parity (`ChildrenContainer.scala:140`):
        // `TerminatingChildrenContainer(c, Set(actor), UserRequest)`.
        let c = core::mem::take(c);
        *self = Self::Terminating { c, to_die: alloc::vec![pid], reason: SuspendReason::UserRequest };
      },
      | Self::Terminating { to_die, .. } => {
        // Pekko parity (`ChildrenContainer.scala:203`):
        // `copy(toDie = toDie + actor)`.
        if !to_die.contains(&pid) {
          to_die.push(pid);
        }
      },
    }
  }

  /// Replaces the termination reason on a [`Terminating`](Self::Terminating)
  /// container.
  ///
  /// Returns `true` iff the current state is [`Terminating`](Self::Terminating)
  /// and the reason was updated. Returns `false` in all other states — Pekko
  /// parity (`Children.scala:178-183`).
  //
  // CQS 違反の根拠:
  // Pekko `setChildrenTerminationReason` は `@tailrec` で CAS ループを回し
  // 成功/失敗（= Terminating だったか否か）を `Boolean` で返す。
  // fraktor-rs では `SpinSyncMutex` 配下で単発の条件付き書き込みに畳んで
  // いるが、呼び出し側 (`AC-H4 finish_recreate`) が「遷移が起きたか？」を
  // 判定する必要があるため、`&mut self` + 戻り値 (`Vec::pop` 相当例外) を
  // 許容する。`cqs-principle.md` の判定フロー「ロジック上分離不可のため
  // CQS 違反を許容」に該当し、人間の許可を取得済み (plan § 確認事項)。
  //
  // AC-H4 `finish_recreate` / `finish_terminate` で配線予定。現時点では test
  // 経由でのみ到達するため `#[allow(dead_code)]` を付与する。
  #[allow(dead_code)]
  pub(crate) fn set_children_termination_reason(&mut self, reason: SuspendReason) -> bool {
    match self {
      | Self::Terminating { reason: current, .. } => {
        *current = reason;
        true
      },
      | _ => false,
    }
  }

  /// Removes `pid` from the container and returns the state-change reason, if
  /// any.
  ///
  /// Pekko parity (`Children.scala:240-257`): only
  /// [`Terminating`](Self::Terminating) produces a meaningful return value.
  ///
  /// * For [`Normal`](Self::Normal) the pid is dropped (Empty if it was the
  ///   last one) and `None` is returned.
  /// * For [`Terminating`](Self::Terminating):
  ///   * If removing `pid` from `to_die` keeps it non-empty, the state stays
  ///     [`Terminating`](Self::Terminating) and `None` is returned.
  ///   * Otherwise:
  ///     * `reason == Termination` → transition to
  ///       [`Terminated`](Self::Terminated), return `Some(Termination)`.
  ///     * `reason != Termination` → transition to
  ///       [`Normal`](Self::Normal) (or [`Empty`](Self::Empty) if no children
  ///       remain), return `Some(reason)`.
  /// * [`Empty`](Self::Empty) / [`Terminated`](Self::Terminated) return `None`.
  //
  // CQS 違反の根拠:
  // Pekko `removeChildAndGetStateChange` は CAS ループ内で状態遷移と
  // 「遷移後に観測された reason」を原子的に返す。fraktor-rs でも呼び出し側
  // (`handle_terminated` / `AC-H4 finish_terminate`) が「Terminating が解消
  // したか？」を判定する必要があり、分離すると呼び出し側で再 match が発生して
  // TOCTOU 警戒の対象になるため、`Vec::pop` 相当の例外として CQS 違反を
  // 許容する。人間の許可を取得済み (plan § 確認事項)。
  pub(crate) fn remove_child_and_get_state_change(&mut self, pid: Pid) -> Option<SuspendReason> {
    match self {
      | Self::Empty | Self::Terminated => None,
      | Self::Normal { c } => {
        remove_child_from_vec(c, pid);
        if c.is_empty() {
          *self = Self::Empty;
        }
        None
      },
      | Self::Terminating { c, to_die, reason } => {
        // Pekko parity (`ChildrenContainer.scala:181-188`): compute the next
        // container *after* removing `pid` from both `c` and `toDie`.
        remove_child_from_vec(c, pid);
        to_die.retain(|existing| *existing != pid);
        if to_die.is_empty() {
          // Transition out of Terminating.
          match reason {
            | SuspendReason::Termination => {
              *self = Self::Terminated;
              Some(SuspendReason::Termination)
            },
            | _ => {
              // Preserve the reason to return it after we mutate `self`.
              let returned_reason = reason.clone();
              let remaining = core::mem::take(c);
              *self = if remaining.is_empty() { Self::Empty } else { Self::Normal { c: remaining } };
              Some(returned_reason)
            },
          }
        } else {
          // Still waiting for more children to die.
          None
        }
      },
    }
  }

  /// Returns every registered child pid in insertion order.
  ///
  /// Pekko parity (`ChildrenContainer.scala:134-135`, `197-198`): only
  /// [`Normal`](Self::Normal) / [`Terminating`](Self::Terminating) produce a
  /// non-empty iterable; [`Empty`](Self::Empty) / [`Terminated`](Self::Terminated)
  /// return an empty sequence.
  #[must_use]
  pub(crate) fn children(&self) -> Vec<Pid> {
    match self {
      | Self::Empty | Self::Terminated => Vec::new(),
      | Self::Normal { c } | Self::Terminating { c, .. } => c.iter().map(|(pid, _)| *pid).collect(),
    }
  }

  /// Returns the restart statistics registered for `pid`, if any.
  ///
  /// Pekko parity (`ChildrenContainer.scala:129-132`, `192-195`): only
  /// [`Normal`](Self::Normal) / [`Terminating`](Self::Terminating) hold stats;
  /// other states return `None`.
  #[must_use]
  pub(crate) fn stats_for(&self, pid: Pid) -> Option<&RestartStatistics> {
    match self {
      | Self::Empty | Self::Terminated => None,
      | Self::Normal { c } | Self::Terminating { c, .. } => {
        c.iter().find(|(existing, _)| *existing == pid).map(|(_, stats)| stats)
      },
    }
  }

  /// Returns a mutable reference to the restart statistics registered for
  /// `pid`, inserting a fresh entry if the pid is not yet known.
  ///
  /// * [`Empty`](Self::Empty) promotes to [`Normal`](Self::Normal) and
  ///   registers `pid`.
  /// * [`Normal`](Self::Normal) / [`Terminating`](Self::Terminating) return
  ///   the existing entry or insert a fresh one.
  /// * [`Terminated`](Self::Terminated) returns `None` — the container is
  ///   sealed and the caller must handle the absence explicitly.
  #[must_use]
  pub(crate) fn stats_for_mut(&mut self, pid: Pid) -> Option<&mut RestartStatistics> {
    match self {
      | Self::Empty => {
        *self = Self::Normal { c: alloc::vec![(pid, RestartStatistics::new())] };
        match self {
          | Self::Normal { c } => c.iter_mut().find(|(existing, _)| *existing == pid).map(|(_, stats)| stats),
          | _ => None,
        }
      },
      | Self::Normal { c } | Self::Terminating { c, .. } => {
        let index = match c.iter().position(|(existing, _)| *existing == pid) {
          | Some(index) => index,
          | None => {
            c.push((pid, RestartStatistics::new()));
            c.len() - 1
          },
        };
        Some(&mut c[index].1)
      },
      | Self::Terminated => None,
    }
  }

  /// Pekko parity (`ChildrenContainer.scala:44`, `110`, `219`):
  ///
  /// * [`Empty`](Self::Empty) → `true`
  /// * [`Normal`](Self::Normal) → `true`
  /// * [`Terminating`](Self::Terminating) → `true` iff
  ///   `reason == SuspendReason::UserRequest`
  /// * [`Terminated`](Self::Terminated) → `false`
  //
  // AC-H2 で `ActorCell::children_state_is_normal` / Pekko `handleFailure` の
  // `!isNormal` 分岐に配線済み。
  #[must_use]
  pub(crate) fn is_normal(&self) -> bool {
    match self {
      | Self::Empty | Self::Normal { .. } => true,
      | Self::Terminating { reason, .. } => matches!(reason, SuspendReason::UserRequest),
      | Self::Terminated => false,
    }
  }

  /// Pekko parity (`ChildrenContainer.scala:43`, `109`, `218`):
  ///
  /// * [`Terminated`](Self::Terminated) → `true`
  /// * [`Terminating`](Self::Terminating) → `true` iff
  ///   `reason == SuspendReason::Termination`
  /// * Other states → `false`
  //
  // AC-H2 で `ActorCell::children_state_is_terminating` / Pekko `terminate` の
  // 判定経路に配線済み。
  #[must_use]
  pub(crate) fn is_terminating(&self) -> bool {
    match self {
      | Self::Terminated => true,
      | Self::Terminating { reason, .. } => matches!(reason, SuspendReason::Termination),
      | Self::Empty | Self::Normal { .. } => false,
    }
  }
}

/// Inserts `pid` into the child registry if absent. No-op for duplicates.
fn insert_child_if_absent(entries: &mut Vec<(Pid, RestartStatistics)>, pid: Pid) {
  if entries.iter().any(|(existing, _)| *existing == pid) {
    return;
  }
  entries.push((pid, RestartStatistics::new()));
}

/// Removes `pid` from the child registry. No-op if not found.
fn remove_child_from_vec(entries: &mut Vec<(Pid, RestartStatistics)>, pid: Pid) {
  entries.retain(|(existing, _)| *existing != pid);
}
