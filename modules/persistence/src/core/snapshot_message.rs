//! Messages sent to snapshot actors.

#[cfg(test)]
mod tests;

use alloc::string::String;

use fraktor_actor_rs::core::actor::actor_ref::ActorRefGeneric;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{snapshot_metadata::SnapshotMetadata, snapshot_selection_criteria::SnapshotSelectionCriteria};

/// Messages sent to the snapshot actor.
#[derive(Clone, Debug)]
pub enum SnapshotMessage<TB: RuntimeToolbox + 'static> {
  /// Saves a snapshot.
  SaveSnapshot {
    /// Snapshot metadata.
    metadata: SnapshotMetadata,
    /// Snapshot payload.
    snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
    /// Request sender.
    sender:   ActorRefGeneric<TB>,
  },
  /// Loads a snapshot.
  LoadSnapshot {
    /// Persistence id to load.
    persistence_id: String,
    /// Selection criteria.
    criteria:       SnapshotSelectionCriteria,
    /// Request sender.
    sender:         ActorRefGeneric<TB>,
  },
  /// Deletes a single snapshot.
  DeleteSnapshot {
    /// Snapshot metadata.
    metadata: SnapshotMetadata,
    /// Request sender.
    sender:   ActorRefGeneric<TB>,
  },
  /// Deletes snapshots by criteria.
  DeleteSnapshots {
    /// Persistence id to delete.
    persistence_id: String,
    /// Selection criteria.
    criteria:       SnapshotSelectionCriteria,
    /// Request sender.
    sender:         ActorRefGeneric<TB>,
  },
}
