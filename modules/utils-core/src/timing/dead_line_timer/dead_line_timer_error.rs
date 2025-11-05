use core::fmt;

#[cfg(test)]
mod tests;

/// Errors that may occur during DeadLineTimer operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadLineTimerError {
  /// The entry corresponding to the specified key does not exist.
  KeyNotFound,
  /// The DeadlineTimer cannot be operated on (e.g., already stopped).
  Closed,
}

impl fmt::Display for DeadLineTimerError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | DeadLineTimerError::KeyNotFound => write!(f, "key not found"),
      | DeadLineTimerError::Closed => write!(f, "deadline timer is closed"),
    }
  }
}

impl core::error::Error for DeadLineTimerError {}
