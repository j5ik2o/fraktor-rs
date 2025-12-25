//! Tests for AskError.

use super::AskError;

#[test]
fn timeout_has_correct_display() {
  let error = AskError::Timeout;
  let message = alloc::format!("{error}");
  assert!(message.contains("Timeout"), "display should mention timeout");
}

#[test]
fn dead_letter_has_correct_display() {
  let error = AskError::DeadLetter;
  let message = alloc::format!("{error}");
  assert!(message.contains("DeadLetter"), "display should mention dead letter");
}

#[test]
fn send_failed_has_correct_display() {
  let error = AskError::SendFailed;
  let message = alloc::format!("{error}");
  assert!(message.contains("SendFailed"), "display should mention send failed");
}

#[test]
fn ask_error_implements_copy() {
  let error = AskError::Timeout;
  let copied = error;
  assert_eq!(error, copied);
}

#[test]
fn ask_error_implements_debug() {
  let error = AskError::Timeout;
  let debug_str = alloc::format!("{error:?}");
  assert!(!debug_str.is_empty());
}

#[test]
fn ask_error_variants_are_distinct() {
  let timeout = AskError::Timeout;
  let dead_letter = AskError::DeadLetter;
  let send_failed = AskError::SendFailed;

  assert_ne!(timeout, dead_letter);
  assert_ne!(timeout, send_failed);
  assert_ne!(dead_letter, send_failed);
}
