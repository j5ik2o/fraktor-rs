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
//! Mapping notes:
//! * `UserRequest` — set by [`ChildrenContainer::shall_die`] when the user explicitly calls
//!   `ActorContext::stop(child)`. The parent remains in its normal lifecycle.
//! * `Recreation(cause)` — set by `fault_recreate` via
//!   [`ChildrenContainer::set_children_termination_reason`] to drive AC-H4 `finish_recreate` once
//!   all children terminate.
//! * `Termination` — set when the parent itself is terminating; transitions the container to
//!   `Terminated` after the last child dies.
//!
//! Pekko's `Creation` variant (used for the `pre_start` handshake) is intentionally omitted
//! until the corresponding code path is ported (YAGNI).

#[cfg(test)]
mod tests;

use crate::actor::error::ActorErrorReason;

/// Reason tagged onto a `TerminatingChildrenContainer` while it waits for its
/// outstanding children to die.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SuspendReason {
  /// The user explicitly called `context.stop(child)`. The parent is still
  /// operating normally; the container reports `isNormal = true` in this case
  /// (Pekko parity).
  UserRequest,
  /// The parent is restarting itself and is waiting for its children to die
  /// before re-creating the actor instance. Mixes in `WaitingForChildren`.
  Recreation(ActorErrorReason),
  /// The parent is terminating. Once all outstanding children die, the
  /// container transitions to `Terminated`.
  Termination,
}
