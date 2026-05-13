//! Scheduler error types returned by public APIs.

use core::fmt::{Display, Formatter, Result as FmtResult};

#[cfg(test)]
#[path = "error_test.rs"]
mod tests;

/// Errors raised when scheduling requests fail.
#[derive(Debug, PartialEq, Eq)]
pub enum SchedulerError {
  /// Delay or period was zero/negative or overflowed supported range.
  InvalidDelay,
  /// The actor cell associated with the timer handle is no longer available.
  ActorUnavailable,
  /// Scheduler has been shut down and no longer accepts jobs.
  Closed,
  /// Scheduler backpressure guard rejected the request.
  Backpressured,
  /// Internal storage reached configured capacity.
  CapacityExceeded,
  /// Shutdown-task queue reached configured capacity.
  TaskRunCapacityExceeded,
}

impl Display for SchedulerError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::InvalidDelay => write!(f, "invalid delay or period"),
      | Self::ActorUnavailable => write!(f, "actor cell unavailable"),
      | Self::Closed => write!(f, "scheduler closed"),
      | Self::Backpressured => write!(f, "scheduler backpressured"),
      | Self::CapacityExceeded => write!(f, "scheduler capacity exceeded"),
      | Self::TaskRunCapacityExceeded => write!(f, "scheduler task-run capacity exceeded"),
    }
  }
}
