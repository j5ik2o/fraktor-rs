//! Signals forwarded to typed behaviors.

use crate::core::{
  kernel::actor::{Pid, error::ActorError},
  typed::message_adapter::AdapterError,
};

/// Enumerates lifecycle notifications delivered to typed behaviors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BehaviorSignal {
  /// Indicates that the actor has completed post-stop processing.
  PostStop,
  /// Indicates that a watched actor terminated with the provided pid.
  Terminated(Pid),
  /// Indicates that message adaptation failed before reaching the behavior.
  MessageAdaptionFailure(AdapterError),
  /// Indicates that a child actor failed with the provided pid and error.
  ChildFailed {
    /// Pid of the child actor that failed.
    pid:   Pid,
    /// Error that caused the child to fail.
    error: ActorError,
  },
  /// Indicates that the actor is about to be restarted by its supervisor.
  PreRestart,
}
