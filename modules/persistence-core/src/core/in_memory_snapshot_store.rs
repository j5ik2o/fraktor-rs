//! In-memory snapshot store implementation for testing.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};
use core::future::{Ready, ready};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  snapshot::Snapshot, snapshot_error::SnapshotError, snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria, snapshot_store::SnapshotStore,
};

/// In-memory snapshot store implementation.
#[derive(Clone, Debug, Default)]
pub struct InMemorySnapshotStore {
  snapshots: BTreeMap<String, Vec<Snapshot>>,
}

impl InMemorySnapshotStore {
  /// Creates a new in-memory snapshot store.
  #[must_use]
  pub const fn new() -> Self {
    Self { snapshots: BTreeMap::new() }
  }

  fn select_latest(&self, persistence_id: &str, criteria: &SnapshotSelectionCriteria) -> Option<Snapshot> {
    let entries = self.snapshots.get(persistence_id)?;
    entries
      .iter()
      .filter(|snapshot| criteria.matches(snapshot.metadata()))
      .max_by(|left, right| {
        let left_metadata = left.metadata();
        let right_metadata = right.metadata();
        left_metadata
          .sequence_nr()
          .cmp(&right_metadata.sequence_nr())
          .then_with(|| left_metadata.timestamp().cmp(&right_metadata.timestamp()))
      })
      .cloned()
  }
}

impl SnapshotStore for InMemorySnapshotStore {
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
    metadata: SnapshotMetadata,
    snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    let entry = self.snapshots.entry(metadata.persistence_id().to_string()).or_default();
    entry.push(Snapshot::new(metadata, snapshot));
    ready(Ok(()))
  }

  fn load_snapshot<'a>(&'a self, persistence_id: &'a str, criteria: SnapshotSelectionCriteria) -> Self::LoadFuture<'a> {
    ready(Ok(self.select_latest(persistence_id, &criteria)))
  }

  fn delete_snapshot<'a>(&'a mut self, metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    if let Some(entries) = self.snapshots.get_mut(metadata.persistence_id()) {
      entries.retain(|snapshot| snapshot.metadata() != metadata);
      if entries.is_empty() {
        self.snapshots.remove(metadata.persistence_id());
      }
    }
    ready(Ok(()))
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    persistence_id: &'a str,
    criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    if let Some(entries) = self.snapshots.get_mut(persistence_id) {
      entries.retain(|snapshot| !criteria.matches(snapshot.metadata()));
      if entries.is_empty() {
        self.snapshots.remove(persistence_id);
      }
    }
    ready(Ok(()))
  }
}
