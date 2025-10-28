use core::fmt;

/// Errors that may occur during DeadlineTimer operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadlineTimerError {
  /// The entry corresponding to the specified key does not exist.
  KeyNotFound,
  /// The DeadlineTimer cannot be operated on (e.g., already stopped).
  Closed,
}

impl fmt::Display for DeadlineTimerError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | DeadlineTimerError::KeyNotFound => write!(f, "key not found"),
      | DeadlineTimerError::Closed => write!(f, "deadline timer is closed"),
    }
  }
}

impl core::error::Error for DeadlineTimerError {}
