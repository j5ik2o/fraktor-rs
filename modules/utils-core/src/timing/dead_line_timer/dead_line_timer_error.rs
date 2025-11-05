use core::fmt;

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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn deadline_timer_error_key_not_found_variant() {
    let error = DeadLineTimerError::KeyNotFound;
    assert_eq!(error, DeadLineTimerError::KeyNotFound);
  }

  #[test]
  fn deadline_timer_error_closed_variant() {
    let error = DeadLineTimerError::Closed;
    assert_eq!(error, DeadLineTimerError::Closed);
  }

  #[test]
  fn deadline_timer_error_clone() {
    let original = DeadLineTimerError::KeyNotFound;
    let cloned = original.clone();
    assert_eq!(original, cloned);
  }

  #[test]
  fn deadline_timer_error_copy() {
    let original = DeadLineTimerError::Closed;
    let copied = original;
    assert_eq!(original, copied);
  }

  #[test]
  fn deadline_timer_error_debug() {
    let error = DeadLineTimerError::KeyNotFound;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("KeyNotFound"));
  }

  #[test]
  fn deadline_timer_error_partial_eq() {
    assert_eq!(DeadLineTimerError::KeyNotFound, DeadLineTimerError::KeyNotFound);
    assert_eq!(DeadLineTimerError::Closed, DeadLineTimerError::Closed);
    assert_ne!(DeadLineTimerError::KeyNotFound, DeadLineTimerError::Closed);
  }

  #[test]
  fn deadline_timer_error_display_key_not_found() {
    let error = DeadLineTimerError::KeyNotFound;
    let display_str = format!("{}", error);
    assert_eq!(display_str, "key not found");
  }

  #[test]
  fn deadline_timer_error_display_closed() {
    let error = DeadLineTimerError::Closed;
    let display_str = format!("{}", error);
    assert_eq!(display_str, "deadline timer is closed");
  }
}
