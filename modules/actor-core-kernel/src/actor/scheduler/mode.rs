//! Supported scheduling modes.

/// Execution mode assigned to scheduled jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulerMode {
  /// Single-shot execution.
  OneShot,
  /// Fixed-rate execution based on initial deadline.
  FixedRate,
  /// Fixed-delay execution measured from handler completion.
  FixedDelay,
}
