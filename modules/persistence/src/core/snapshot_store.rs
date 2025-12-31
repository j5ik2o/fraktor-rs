//! Snapshot store abstraction.

use core::future::Future;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  snapshot::Snapshot, snapshot_error::SnapshotError, snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria,
};

/// Snapshot store abstraction using GATs for no_std async.
pub trait SnapshotStore: Send + Sync + 'static {
  /// Future returned by save operations.
  type SaveFuture<'a>: Future<Output = Result<(), SnapshotError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by load operations.
  type LoadFuture<'a>: Future<Output = Result<Option<Snapshot>, SnapshotError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by single delete operations.
  type DeleteOneFuture<'a>: Future<Output = Result<(), SnapshotError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by bulk delete operations.
  type DeleteManyFuture<'a>: Future<Output = Result<(), SnapshotError>> + Send + 'a
  where
    Self: 'a;

  /// Saves a snapshot.
  fn save_snapshot<'a>(
    &'a mut self,
    metadata: SnapshotMetadata,
    snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
  ) -> Self::SaveFuture<'a>;

  /// Loads a snapshot using the provided criteria.
  fn load_snapshot<'a>(&'a self, persistence_id: &'a str, criteria: SnapshotSelectionCriteria) -> Self::LoadFuture<'a>;

  /// Deletes a single snapshot.
  fn delete_snapshot<'a>(&'a mut self, metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a>;

  /// Deletes snapshots matching the criteria.
  fn delete_snapshots<'a>(
    &'a mut self,
    persistence_id: &'a str,
    criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a>;
}
