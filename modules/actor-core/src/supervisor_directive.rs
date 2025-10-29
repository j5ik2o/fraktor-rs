//! Decision emitted by a supervisor strategy when handling an error.

/// Decision emitted by a supervisor strategy when handling an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorDirective {
  /// Resume processing without restarting the actor.
  Resume,
  /// Restart the failing actor.
  Restart,
  /// Stop the failing actor.
  Stop,
  /// Escalate to the parent supervisor.
  Escalate,
}
