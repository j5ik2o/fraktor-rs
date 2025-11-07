//! Internal system messages exchanged within the actor runtime.

#[cfg(test)]
mod tests;

pub use self::{
  failure_classification::FailureClassification, failure_message_snapshot::FailureMessageSnapshot,
  failure_payload::FailurePayload,
};
use crate::{RuntimeToolbox, actor_prim::Pid, messaging::AnyMessageGeneric};

mod failure_classification;
mod failure_message_snapshot;
mod failure_payload;

/// Lightweight enum describing system-level mailbox traffic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemMessage {
  /// Signals that the associated actor should stop.
  Stop,
  /// Requests actor initialization via the mailbox pipeline.
  Create,
  /// Recreates the actor instance after a recoverable failure.
  Recreate,
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
  /// Notifies watchers that the referenced actor has terminated.
  Terminated(Pid),
  /// Reports that a child actor failed and requires supervisor handling.
  Failure(FailurePayload),
}

impl<TB: RuntimeToolbox> From<SystemMessage> for AnyMessageGeneric<TB> {
  fn from(value: SystemMessage) -> Self {
    AnyMessageGeneric::new(value)
  }
}
