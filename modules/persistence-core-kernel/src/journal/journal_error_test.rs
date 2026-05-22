use alloc::string::ToString;

use crate::journal::JournalError;

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
fn journal_error_display_invalid_atomic_write() {
  let error = JournalError::InvalidAtomicWrite("empty payload".into());

  assert_eq!(error.to_string(), "invalid atomic write: empty payload");
}

#[test]
fn journal_error_display_mixed_persistence_id() {
  let error = JournalError::MixedPersistenceId { expected: "pid-1".into(), actual: "pid-2".into() };

  assert_eq!(error.to_string(), "mixed persistence id: expected pid-1, actual pid-2");
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
