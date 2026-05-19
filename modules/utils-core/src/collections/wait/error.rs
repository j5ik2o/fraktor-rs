use core::fmt::{Display, Formatter, Result as FmtResult};

/// Errors that can occur when registering a waiter in a WaitQueue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitError {
  /// The queue failed to allocate memory for a new waiter.
  AllocationFailure,
  /// The queue is closed and cannot accept new waiters.
  QueueClosed,
}

impl Display for WaitError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::AllocationFailure => write!(f, "failed to allocate memory for waiter"),
      | Self::QueueClosed => write!(f, "queue is closed and cannot accept new waiters"),
    }
  }
}
