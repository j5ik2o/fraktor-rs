//! Responses emitted by snapshot actors.

#[cfg(test)]
mod tests;

use crate::core::{
  snapshot::Snapshot, snapshot_error::SnapshotError, snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria,
};

/// Responses from snapshot operations.
#[derive(Clone, Debug)]
pub enum SnapshotResponse {
  /// Snapshot save succeeded.
  SaveSnapshotSuccess {
    /// Saved snapshot metadata.
    metadata: SnapshotMetadata,
  },
  /// Snapshot save failed.
  SaveSnapshotFailure {
    /// Snapshot metadata.
    metadata: SnapshotMetadata,
    /// Failure cause.
    error:    SnapshotError,
  },
  /// Snapshot load result.
  LoadSnapshotResult {
    /// Loaded snapshot.
    snapshot:       Option<Snapshot>,
    /// Upper bound sequence number used for load.
    to_sequence_nr: u64,
  },
  /// Snapshot load failed.
  LoadSnapshotFailed {
    /// Failure cause.
    error: SnapshotError,
  },
  /// Snapshot delete succeeded.
  DeleteSnapshotSuccess {
    /// Deleted snapshot metadata.
    metadata: SnapshotMetadata,
  },
  /// Snapshot delete by criteria succeeded.
  DeleteSnapshotsSuccess {
    /// Deletion criteria.
    criteria: SnapshotSelectionCriteria,
  },
  /// Snapshot delete failed.
  DeleteSnapshotFailure {
    /// Snapshot metadata.
    metadata: SnapshotMetadata,
    /// Failure cause.
    error:    SnapshotError,
  },
  /// Snapshot delete by criteria failed.
  DeleteSnapshotsFailure {
    /// Deletion criteria.
    criteria: SnapshotSelectionCriteria,
    /// Failure cause.
    error:    SnapshotError,
  },
}
