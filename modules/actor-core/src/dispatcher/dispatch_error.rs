//! Errors returned by dispatch executors.

use core::fmt;

/// Error conditions produced when scheduling dispatcher batches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchError {
  /// Executor is temporarily unable to accept new work.
  RejectedExecution,
  /// Executor cannot be used (e.g. shutdown).
  ExecutorUnavailable,
}

impl fmt::Display for DispatchError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | DispatchError::RejectedExecution => f.write_str("rejected execution"),
      | DispatchError::ExecutorUnavailable => f.write_str("executor unavailable"),
    }
  }
}
