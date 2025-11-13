//! Scheduler error types returned by public APIs.

use core::fmt;

/// Errors raised when scheduling requests fail.
#[derive(Debug, PartialEq, Eq)]
pub enum SchedulerError {
  /// Delay or period was zero/negative or overflowed supported range.
  InvalidDelay,
  /// Scheduler has been shut down and no longer accepts jobs.
  Closed,
  /// Scheduler backpressure guard rejected the request.
  Backpressured,
  /// Internal storage reached configured capacity.
  CapacityExceeded,
}

impl fmt::Display for SchedulerError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::InvalidDelay => write!(f, "invalid delay or period"),
      | Self::Closed => write!(f, "scheduler closed"),
      | Self::Backpressured => write!(f, "scheduler backpressured"),
      | Self::CapacityExceeded => write!(f, "scheduler capacity exceeded"),
    }
  }
}
