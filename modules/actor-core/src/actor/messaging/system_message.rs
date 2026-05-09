//! Internal system messages exchanged within the actor runtime.

#[cfg(test)]
mod tests;

use crate::actor::{Pid, context_pipe::ContextPipeTaskId, error::ActorErrorReason, messaging::AnyMessage};

mod failure_classification;
mod failure_message_snapshot;
mod failure_payload;

pub use failure_classification::FailureClassification;
pub use failure_message_snapshot::FailureMessageSnapshot;
pub use failure_payload::FailurePayload;

/// Lightweight enum describing system-level mailbox traffic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemMessage {
  /// Requests immediate actor termination using PoisonPill semantics.
  PoisonPill,
  /// Requests immediate actor termination using Kill semantics.
  Kill,
  /// Signals that the associated actor should stop.
  Stop,
  /// Requests actor initialization via the mailbox pipeline.
  Create,
  /// Recreates the actor instance after a recoverable failure.
  ///
  /// Carries the failure cause (Pekko `Recreate(cause: Throwable)`).
  /// AC-H4: the payload is preserved through the restart pipeline and
  /// delivered to `pre_restart(reason)` / `post_restart(reason)`.
  Recreate(ActorErrorReason),
  /// Requests the mailbox to suspend user message processing.
  Suspend,
  /// Requests the mailbox to resume user message processing.
  Resume,
  /// Registers the specified watcher for termination notifications.
  Watch(Pid),
  /// Removes the specified watcher and stops sending notifications.
  Unwatch(Pid),
  /// Requests the guardian to stop a specific child actor.
  StopChild(Pid),
  /// Kernel-internal DeathWatch notification (Pekko
  /// `DeathWatch.scala:DeathWatchNotification`).
  ///
  /// AC-H5: this is the sole kernel-internal envelope used to propagate a
  /// terminated actor's notification to its watchers. The watcher's kernel is
  /// responsible for applying `watching_contains_pid` / `terminated_queued`
  /// dedup and then dispatching either a `watch_with` custom message or
  /// [`Actor::on_terminated`] directly.
  DeathWatchNotification(Pid),
  /// Reports that a child actor failed and requires supervisor handling.
  Failure(FailurePayload),
  /// Resumes a pending pipe task once its future has been woken.
  PipeTask(ContextPipeTaskId),
}

impl From<SystemMessage> for AnyMessage {
  fn from(value: SystemMessage) -> Self {
    AnyMessage::new(value)
  }
}
