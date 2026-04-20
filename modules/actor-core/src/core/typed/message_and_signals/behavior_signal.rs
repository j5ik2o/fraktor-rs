//! Signals forwarded to typed behaviors.

use crate::core::typed::message_and_signals::{ChildFailed, MessageAdaptionFailure, Signal, Terminated};

/// Enumerates lifecycle notifications delivered to typed behaviors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BehaviorSignal {
  /// Delivered to a behavior so it can run post-stop logic as the actor is stopping.
  PostStop,
  /// Indicates that a watched actor terminated.
  Terminated(Terminated),
  /// Indicates that message adaptation failed before reaching the behavior.
  MessageAdaptionFailure(MessageAdaptionFailure),
  /// Indicates that a child actor failed.
  ChildFailed(ChildFailed),
  /// Indicates that the actor is about to be restarted by its supervisor.
  PreRestart,
  /// Indicates that the actor has just been restarted by its supervisor.
  PostRestart,
}

impl Signal for BehaviorSignal {}
