/// Decision returned by a supervisor decider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorDecision {
  /// Restart the failing actor.
  Restart,
  /// Stop the failing actor.
  Stop,
  /// Escalate the failure to the parent supervisor.
  Escalate,
}
