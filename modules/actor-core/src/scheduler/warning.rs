//! Warnings emitted by the scheduler for observability.

/// Warning categories tracked inside the scheduler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulerWarning {
  /// Periodic backlog exceeded the configured limit and job was cancelled.
  BacklogExceeded {
    /// Identifier of the cancelled handle.
    handle_id: u64,
    /// Missed runs accumulated before cancellation.
    missed:    u32,
  },
  /// Periodic backlog surpassed the burst threshold but job continues.
  BurstFire {
    /// Identifier of the affected handle.
    handle_id: u64,
    /// Number of missed runs observed.
    missed:    u32,
  },
}
