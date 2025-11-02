/// Directive returned by supervisor strategies when handling failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorDirective {
  /// Restart the failing actor.
  Restart,
  /// Stop the failing actor permanently.
  Stop,
  /// Escalate the failure to the parent supervisor.
  Escalate,
}
