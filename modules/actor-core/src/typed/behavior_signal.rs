//! Signals forwarded to typed behaviors.

use crate::actor_prim::Pid;

/// Enumerates lifecycle notifications delivered to typed behaviors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorSignal {
  /// Indicates that the actor finished the startup sequence.
  Started,
  /// Indicates that the actor is about to stop permanently.
  Stopped,
  /// Indicates that a watched actor terminated with the provided pid.
  Terminated(Pid),
}
