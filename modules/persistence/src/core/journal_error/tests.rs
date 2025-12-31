use alloc::string::ToString;

use crate::core::journal_error::JournalError;

#[test]
fn journal_error_display_sequence_mismatch() {
  let error = JournalError::SequenceMismatch { expected: 2, actual: 3 };

  assert_eq!(error.to_string(), "sequence mismatch: expected 2, actual 3");
}

#[test]
fn journal_error_display_write_failed() {
  let error = JournalError::WriteFailed("io error".into());

  assert_eq!(error.to_string(), "write failed: io error");
}

#[test]
fn journal_error_display_read_failed() {
  let error = JournalError::ReadFailed("decode error".into());

  assert_eq!(error.to_string(), "read failed: decode error");
}

#[test]
fn journal_error_display_delete_failed() {
  let error = JournalError::DeleteFailed("permission denied".into());

  assert_eq!(error.to_string(), "delete failed: permission denied");
}
