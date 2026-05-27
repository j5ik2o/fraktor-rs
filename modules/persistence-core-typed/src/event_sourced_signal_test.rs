use alloc::string::ToString;

use fraktor_persistence_core_kernel_rs::{
  error::PersistenceError,
  snapshot::{SnapshotError, SnapshotMetadata, SnapshotSelectionCriteria},
};

use crate::{EventRejectedError, EventSourcedSignal, PersistenceId};

#[test]
fn journal_failure_signal_holds_error() {
  let error = PersistenceError::StateMachine("write failed".to_string());

  let signal = EventSourcedSignal::JournalPersistFailed { error: error.clone() };

  assert!(matches!(signal, EventSourcedSignal::JournalPersistFailed { error: actual } if actual == error));
}

#[test]
fn journal_rejection_signal_holds_rejected_error() {
  let error = EventRejectedError::new(
    PersistenceId::of_unique_id("pid-rejected"),
    11,
    PersistenceError::StateMachine("write rejected".to_string()),
  );

  let signal = EventSourcedSignal::JournalPersistRejected { error: error.clone() };

  assert!(matches!(signal, EventSourcedSignal::JournalPersistRejected { error: actual } if actual == error));
}

#[test]
fn recovery_snapshot_and_delete_signals_hold_public_pekko_family_payloads() {
  let metadata = SnapshotMetadata::new("pid-signal", 4, 10);
  let criteria = SnapshotSelectionCriteria::latest();
  let recovery_error = PersistenceError::Recovery("recovery failed".to_string());
  let snapshot_error = PersistenceError::from(SnapshotError::SaveFailed("snapshot failed".to_string()));
  let delete_error = PersistenceError::from(SnapshotError::DeleteFailed("delete failed".to_string()));
  let journal_error = PersistenceError::StateMachine("delete events failed".to_string());

  assert!(matches!(EventSourcedSignal::RecoveryCompleted, EventSourcedSignal::RecoveryCompleted));
  assert!(matches!(EventSourcedSignal::RecoveryFailed { error: recovery_error.clone() },
    EventSourcedSignal::RecoveryFailed { error } if error == recovery_error));
  assert!(matches!(EventSourcedSignal::SnapshotCompleted { metadata: metadata.clone() },
    EventSourcedSignal::SnapshotCompleted { metadata: actual } if actual == metadata));
  assert!(
    matches!(EventSourcedSignal::SnapshotFailed { metadata: Some(metadata.clone()), error: snapshot_error.clone() },
    EventSourcedSignal::SnapshotFailed { metadata: Some(actual_metadata), error } if actual_metadata == metadata && error == snapshot_error)
  );
  assert!(matches!(EventSourcedSignal::DeleteSnapshotsCompleted { criteria: criteria.clone() },
    EventSourcedSignal::DeleteSnapshotsCompleted { criteria: actual } if actual == criteria));
  assert!(
    matches!(EventSourcedSignal::DeleteSnapshotsFailed { criteria: criteria.clone(), error: delete_error.clone() },
    EventSourcedSignal::DeleteSnapshotsFailed { criteria: actual, error } if actual == criteria && error == delete_error)
  );
  assert!(matches!(
    EventSourcedSignal::DeleteEventsCompleted { to_sequence_nr: 9 },
    EventSourcedSignal::DeleteEventsCompleted { to_sequence_nr: 9 }
  ));
  assert!(matches!(EventSourcedSignal::DeleteEventsFailed { to_sequence_nr: 9, error: journal_error.clone() },
    EventSourcedSignal::DeleteEventsFailed { to_sequence_nr: 9, error } if error == journal_error));
}
