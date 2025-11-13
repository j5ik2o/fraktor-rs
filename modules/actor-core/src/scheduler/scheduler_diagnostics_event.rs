use crate::scheduler::{ExecutionBatch, SchedulerMode, SchedulerWarning};

/// Individual diagnostics events emitted by the scheduler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulerDiagnosticsEvent {
  /// Job registration event with the scheduled deadline.
  Scheduled {
    /// Handle identifier.
    handle_id:     u64,
    /// Deadline tick assigned during registration.
    deadline_tick: u64,
    /// Scheduling mode.
    mode:          SchedulerMode,
  },
  /// Job execution event including batch metadata.
  Fired {
    /// Handle identifier.
    handle_id:  u64,
    /// Tick when the job fired.
    fired_tick: u64,
    /// Execution batch metadata.
    batch:      ExecutionBatch,
  },
  /// Job cancellation notification.
  Cancelled {
    /// Handle identifier.
    handle_id:      u64,
    /// Tick when the cancellation was recorded.
    cancelled_tick: u64,
  },
  /// Warning emitted by the scheduler.
  Warning {
    /// Warning payload.
    warning: SchedulerWarning,
  },
}
