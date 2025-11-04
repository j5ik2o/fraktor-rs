//! Directive returned by supervisor strategies when handling failures.

/// Supervisor directive emitted after evaluating a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorDirective {
  /// Restart the failing actor.
  Restart,
  /// Stop the failing actor permanently.
  Stop,
  /// Escalate the failure to the parent supervisor.
  Escalate,
}
