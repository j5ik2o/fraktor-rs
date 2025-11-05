use super::DeadLineTimerError;

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
  let cloned = original;
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
