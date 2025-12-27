//! In-memory snapshot store implementation.

use alloc::{string::String, vec::Vec};

use ahash::RandomState;
use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::core::{
  snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria,
  snapshot_store::{SnapshotLoadResult, SnapshotStore},
  snapshot_store_error::SnapshotStoreError,
};

/// In-memory snapshot store keeping snapshots per persistence id.
pub struct InMemorySnapshotStore {
  entries: HashMap<String, Vec<StoredSnapshot>, RandomState>,
}

impl InMemorySnapshotStore {
  /// Creates a new empty snapshot store.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()) }
  }
}

impl Default for InMemorySnapshotStore {
  fn default() -> Self {
    Self::new()
  }
}

impl SnapshotStore for InMemorySnapshotStore {
  fn load_snapshot(
    &self,
    persistence_id: &str,
    criteria: SnapshotSelectionCriteria,
    to_sequence_nr: u64,
  ) -> Result<SnapshotLoadResult, SnapshotStoreError> {
    let criteria = criteria.limit(to_sequence_nr);
    let mut best: Option<&StoredSnapshot> = None;
    if let Some(entries) = self.entries.get(persistence_id) {
      for entry in entries {
        if criteria.matches(&entry.metadata) {
          best = match best {
            | None => Some(entry),
            | Some(current) => {
              if entry.metadata.sequence_nr() > current.metadata.sequence_nr()
                || (entry.metadata.sequence_nr() == current.metadata.sequence_nr()
                  && entry.metadata.timestamp() > current.metadata.timestamp())
              {
                Some(entry)
              } else {
                Some(current)
              }
            },
          };
        }
      }
    }
    Ok(best.map(|entry| (entry.metadata.clone(), entry.snapshot.clone())))
  }

  fn save_snapshot(
    &mut self,
    metadata: SnapshotMetadata,
    snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
  ) -> Result<(), SnapshotStoreError> {
    let persistence_id = String::from(metadata.persistence_id());
    let entry = self.entries.entry(persistence_id).or_insert_with(Vec::new);
    entry.push(StoredSnapshot { metadata, snapshot });
    Ok(())
  }

  fn delete_snapshot(&mut self, metadata: &SnapshotMetadata) -> Result<(), SnapshotStoreError> {
    if let Some(entries) = self.entries.get_mut(metadata.persistence_id()) {
      entries.retain(|entry| entry.metadata != *metadata);
    }
    Ok(())
  }

  fn delete_snapshots(
    &mut self,
    persistence_id: &str,
    criteria: SnapshotSelectionCriteria,
  ) -> Result<(), SnapshotStoreError> {
    if let Some(entries) = self.entries.get_mut(persistence_id) {
      entries.retain(|entry| !criteria.matches(&entry.metadata));
    }
    Ok(())
  }
}

struct StoredSnapshot {
  metadata: SnapshotMetadata,
  snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
}
