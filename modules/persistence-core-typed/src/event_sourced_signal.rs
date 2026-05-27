//! Event-sourced behavior signals.

#[cfg(test)]
#[path = "event_sourced_signal_test.rs"]
mod tests;

use fraktor_persistence_core_kernel_rs::{
  error::PersistenceError,
  snapshot::{SnapshotMetadata, SnapshotSelectionCriteria},
};

use crate::EventRejectedError;

/// Public failure signals emitted by event-sourced persistence operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventSourcedSignal {
  /// Recovery completed.
  RecoveryCompleted,
  /// Recovery failed.
  RecoveryFailed {
    /// Error reported by recovery.
    error: PersistenceError,
  },
  /// Journal persist failed.
  JournalPersistFailed {
    /// Error reported by the journal.
    error: PersistenceError,
  },
  /// Journal persist was rejected.
  JournalPersistRejected {
    /// Rejection details reported by the journal.
    error: EventRejectedError,
  },
  /// Snapshot save completed.
  SnapshotCompleted {
    /// Snapshot metadata.
    metadata: SnapshotMetadata,
  },
  /// Snapshot operation failed.
  SnapshotFailed {
    /// Snapshot metadata when the failed operation is tied to one snapshot.
    metadata: Option<SnapshotMetadata>,
    /// Error reported by the snapshot store.
    error:    PersistenceError,
  },
  /// Snapshot deletion completed.
  DeleteSnapshotsCompleted {
    /// Deletion selection criteria.
    criteria: SnapshotSelectionCriteria,
  },
  /// Snapshot deletion failed.
  DeleteSnapshotsFailed {
    /// Deletion selection criteria.
    criteria: SnapshotSelectionCriteria,
    /// Error reported by the snapshot store.
    error:    PersistenceError,
  },
  /// Event deletion completed.
  DeleteEventsCompleted {
    /// Inclusive upper sequence number.
    to_sequence_nr: u64,
  },
  /// Event deletion failed.
  DeleteEventsFailed {
    /// Inclusive upper sequence number.
    to_sequence_nr: u64,
    /// Error reported by the journal.
    error:          PersistenceError,
  },
}
