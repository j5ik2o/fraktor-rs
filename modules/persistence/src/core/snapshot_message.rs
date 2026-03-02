//! Messages sent to snapshot actors.

#[cfg(test)]
mod tests;

use alloc::string::String;

use fraktor_actor_rs::core::actor::actor_ref::ActorRef;
use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{snapshot_metadata::SnapshotMetadata, snapshot_selection_criteria::SnapshotSelectionCriteria};

/// Messages sent to the snapshot actor.
#[derive(Clone, Debug)]
pub enum SnapshotMessage {
  /// Saves a snapshot.
  SaveSnapshot {
    /// Snapshot metadata.
    metadata: SnapshotMetadata,
    /// Snapshot payload.
    snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
    /// Request sender.
    sender:   ActorRef,
  },
  /// Loads a snapshot.
  LoadSnapshot {
    /// Persistence id to load.
    persistence_id: String,
    /// Selection criteria.
    criteria:       SnapshotSelectionCriteria,
    /// Request sender.
    sender:         ActorRef,
  },
  /// Deletes a single snapshot.
  DeleteSnapshot {
    /// Snapshot metadata.
    metadata: SnapshotMetadata,
    /// Request sender.
    sender:   ActorRef,
  },
  /// Deletes snapshots by criteria.
  DeleteSnapshots {
    /// Persistence id to delete.
    persistence_id: String,
    /// Selection criteria.
    criteria:       SnapshotSelectionCriteria,
    /// Request sender.
    sender:         ActorRef,
  },
}
