//! Lifecycle state for delayed dispatcher shutdown.

/// Tracks whether a dispatcher has a pending delayed shutdown.
///
/// The state machine matches Apache Pekko's `MessageDispatcher` shutdown
/// scheduling: a dispatcher transitions from `Unscheduled` to `Scheduled` once
/// the inhabitants count reaches zero, and to `Rescheduled` if a new actor
/// attaches before the delayed shutdown fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownSchedule {
  /// No delayed shutdown is pending.
  Unscheduled,
  /// A delayed shutdown has been scheduled and not yet fired.
  Scheduled,
  /// A delayed shutdown was scheduled but should be cancelled because a new
  /// actor attached. The next delayed-shutdown fire returns to `Unscheduled`.
  Rescheduled,
}
