//! Signals forwarded to typed behaviors.

use crate::{actor_prim::Pid, typed::message_adapter::AdapterFailure};

/// Enumerates lifecycle notifications delivered to typed behaviors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BehaviorSignal {
  /// Indicates that the actor finished the startup sequence.
  Started,
  /// Indicates that the actor is about to stop permanently.
  Stopped,
  /// Indicates that a watched actor terminated with the provided pid.
  Terminated(Pid),
  /// Indicates that message adaptation failed before reaching the behavior.
  AdapterFailed(AdapterFailure),
}
