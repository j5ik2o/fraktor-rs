//! Reason why an actor suspended its mailbox while waiting for child termination.
//!
//! Translated from Pekko `ChildrenContainer.scala:55-77`:
//! ```scala
//! sealed trait SuspendReason
//! case object UserRequest extends SuspendReason
//! final case class Recreation(cause: Throwable) extends SuspendReason with WaitingForChildren
//! final case class Creation() extends SuspendReason with WaitingForChildren
//! case object Termination extends SuspendReason
//! trait WaitingForChildren
//! ```
//!
//! The `WaitingForChildren` mixin in Pekko is exposed here via
//! [`SuspendReason::is_waiting_for_children`] because Rust does not have mixin
//! traits. `Recreation` and `Creation` answer `true`, while `UserRequest` and
//! `Termination` answer `false`.

#[cfg(test)]
mod tests;

use crate::core::kernel::actor::error::ActorErrorReason;

/// Reason tagged onto a `TerminatingChildrenContainer` while it waits for its
/// outstanding children to die.
///
/// See `references/pekko/.../dungeon/ChildrenContainer.scala:55-77` for the
/// original Scala hierarchy.
//
// `Recreation` / `Creation` / `Termination` は AC-H4 (`handle_recreate` /
// `finish_terminate` / `pre_start` 中の子 create) で配線予定。`UserRequest` は
// AC-H2 の `shall_die` で構築済み。AC-H3 は Suspend/Resume の子再帰伝播のみを
// 対象とし SuspendReason を構築しない。現時点では未配線の variant があるため
// enum 全体に `#[allow(dead_code)]` を付与し、AC-H4 で順次解除する。
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SuspendReason {
  /// The user explicitly called `context.stop(child)`. The parent is still
  /// operating normally; the container reports `isNormal = true` in this case.
  UserRequest,
  /// The parent is restarting itself and is waiting for its children to die
  /// before re-creating the actor instance. Mixes in `WaitingForChildren`.
  ///
  /// In Pekko the cause is a `Throwable`; here we use [`ActorErrorReason`]
  /// because fraktor-rs is `no_std` and cannot depend on `std::error::Error`.
  Recreation(ActorErrorReason),
  /// The parent is still inside `pre_start` and must finish creating itself
  /// after its children finish their own `pre_start`. Mixes in
  /// `WaitingForChildren`.
  Creation,
  /// The parent is terminating. Once all outstanding children die, the
  /// container transitions to `Terminated`.
  Termination,
}

impl SuspendReason {
  /// Returns `true` when this reason corresponds to Pekko's `WaitingForChildren`
  /// mixin. `Recreation` and `Creation` return `true`; the other variants return
  /// `false`.
  //
  // AC-H3 / AC-H4 で `handle_recreate` / `finish_terminate` の分岐判定に配線
  // 予定。現時点では test 経由でのみ到達するため `#[allow(dead_code)]` を
  // 付与する。
  #[allow(dead_code)]
  #[must_use]
  pub(crate) const fn is_waiting_for_children(&self) -> bool {
    matches!(self, Self::Recreation(_) | Self::Creation)
  }
}
