//! Failure modes reported by [`Executor::execute`](super::Executor::execute) submissions.

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

#[cfg(test)]
#[path = "execute_error_test.rs"]
mod tests;

/// Errors returned when an executor cannot accept a submitted task.
///
/// `ExecuteError` makes the executor backend's failure observable so that
/// `MessageDispatcherShared::register_for_execution` can roll the mailbox state
/// back to idle and surface the failure via metrics or logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteError {
  /// The executor refused the task without entering shutdown (e.g. saturated queue).
  Rejected,
  /// The executor has already been shut down and no longer accepts tasks.
  Shutdown,
  /// The backend reported an implementation-specific failure.
  Backend(String),
}

impl Display for ExecuteError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::Rejected => f.write_str("executor rejected the submitted task"),
      | Self::Shutdown => f.write_str("executor is shut down"),
      | Self::Backend(reason) => write!(f, "executor backend error: {reason}"),
    }
  }
}

impl core::error::Error for ExecuteError {}
