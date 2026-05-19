//! Snapshot store implementation that never stores snapshots.

#[cfg(test)]
#[path = "no_snapshot_store_test.rs"]
mod tests;

use core::{
  any::Any,
  future::{Ready, ready},
};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::snapshot::{Snapshot, SnapshotError, SnapshotMetadata, SnapshotSelectionCriteria, SnapshotStore};

/// Snapshot store that ignores save and delete requests and always returns no snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoSnapshotStore;

impl NoSnapshotStore {
  /// Creates a no-op snapshot store.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl SnapshotStore for NoSnapshotStore {
  type DeleteManyFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type DeleteOneFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type LoadFuture<'a>
    = Ready<Result<Option<Snapshot>, SnapshotError>>
  where
    Self: 'a;
  type SaveFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;

  fn save_snapshot<'a>(
    &'a mut self,
    _metadata: SnapshotMetadata,
    _snapshot: ArcShared<dyn Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    ready(Ok(()))
  }

  fn load_snapshot<'a>(
    &'a self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::LoadFuture<'a> {
    ready(Ok(None))
  }

  fn delete_snapshot<'a>(&'a mut self, _metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    ready(Ok(()))
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    ready(Ok(()))
  }
}
