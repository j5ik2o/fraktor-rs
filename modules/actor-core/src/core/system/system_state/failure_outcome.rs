//! Supervisor decision outcome after processing a failure.

/// Represents the supervisor decision taken after processing a failure.
#[derive(Clone, Copy, Debug)]
pub enum FailureOutcome {
  /// Indicates the supervisor decided to restart the failed actor.
  Restart,
  /// Indicates the supervisor decided to stop the failed actor.
  Stop,
  /// Indicates the supervisor escalated the failure to its parent.
  Escalate,
}
