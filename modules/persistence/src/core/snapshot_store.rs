//! Snapshot store plugin trait.

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  snapshot_metadata::SnapshotMetadata, snapshot_selection_criteria::SnapshotSelectionCriteria,
  snapshot_store_error::SnapshotStoreError,
};

/// Snapshot load result representation.
pub type SnapshotLoadResult = Option<(SnapshotMetadata, ArcShared<dyn core::any::Any + Send + Sync>)>;

/// Snapshot store interface used by persistent actors.
pub trait SnapshotStore: Send + Sync + 'static {
  /// Loads a snapshot matching the criteria.
  ///
  /// # Errors
  ///
  /// Returns an error when loading fails.
  fn load_snapshot(
    &self,
    persistence_id: &str,
    criteria: SnapshotSelectionCriteria,
    to_sequence_nr: u64,
  ) -> Result<SnapshotLoadResult, SnapshotStoreError>;

  /// Saves a snapshot with metadata.
  ///
  /// # Errors
  ///
  /// Returns an error when saving fails.
  fn save_snapshot(
    &mut self,
    metadata: SnapshotMetadata,
    snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
  ) -> Result<(), SnapshotStoreError>;

  /// Deletes a single snapshot.
  ///
  /// # Errors
  ///
  /// Returns an error when deletion fails.
  fn delete_snapshot(&mut self, metadata: &SnapshotMetadata) -> Result<(), SnapshotStoreError>;

  /// Deletes snapshots matching the provided criteria.
  ///
  /// # Errors
  ///
  /// Returns an error when deletion fails.
  fn delete_snapshots(
    &mut self,
    persistence_id: &str,
    criteria: SnapshotSelectionCriteria,
  ) -> Result<(), SnapshotStoreError>;
}
