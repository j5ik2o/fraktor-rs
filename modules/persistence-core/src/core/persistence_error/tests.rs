use alloc::string::ToString;

use crate::core::{journal_error::JournalError, persistence_error::PersistenceError, snapshot_error::SnapshotError};

#[test]
fn persistence_error_display_journal() {
  let error = PersistenceError::from(JournalError::SequenceMismatch { expected: 1, actual: 3 });

  assert_eq!(error.to_string(), "journal error: sequence mismatch: expected 1, actual 3");
}

#[test]
fn persistence_error_display_snapshot() {
  let error = PersistenceError::from(SnapshotError::LoadFailed("not found".into()));

  assert_eq!(error.to_string(), "snapshot error: load snapshot failed: not found");
}

#[test]
fn persistence_error_display_recovery() {
  let error = PersistenceError::Recovery("broken log".into());

  assert_eq!(error.to_string(), "recovery error: broken log");
}

#[test]
fn persistence_error_display_state_machine() {
  let error = PersistenceError::StateMachine("invalid transition".into());

  assert_eq!(error.to_string(), "state machine error: invalid transition");
}

#[test]
fn persistence_error_display_message_passing() {
  let error = PersistenceError::MessagePassing("sender missing".into());

  assert_eq!(error.to_string(), "message passing error: sender missing");
}
