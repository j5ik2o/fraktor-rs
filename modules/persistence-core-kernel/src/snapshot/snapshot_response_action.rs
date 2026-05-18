//! Actions derived from snapshot responses.

use crate::{
  persistent::Eventsourced,
  snapshot::{Snapshot, SnapshotError, SnapshotMetadata, SnapshotSelectionCriteria},
};

/// Actions to apply on the actor after snapshot response handling.
pub(crate) enum SnapshotResponseAction {
  /// No actor callback required.
  None,
  /// Deliver a loaded snapshot.
  ReceiveSnapshot(Snapshot),
  /// Notify snapshot save success.
  SnapshotSaved(SnapshotMetadata),
  /// Notify single snapshot delete success.
  SnapshotDeleted(SnapshotMetadata),
  /// Notify criteria-based snapshot delete success.
  SnapshotsDeleted(SnapshotSelectionCriteria),
  /// Notify snapshot failure.
  SnapshotFailure(SnapshotError),
}

impl SnapshotResponseAction {
  pub(crate) fn apply(self, actor: &mut impl Eventsourced) {
    match self {
      | SnapshotResponseAction::None => {},
      | SnapshotResponseAction::ReceiveSnapshot(snapshot) => actor.receive_snapshot(&snapshot),
      | SnapshotResponseAction::SnapshotSaved(metadata) => actor.on_snapshot_saved(&metadata),
      | SnapshotResponseAction::SnapshotDeleted(metadata) => actor.on_snapshot_deleted(&metadata),
      | SnapshotResponseAction::SnapshotsDeleted(criteria) => actor.on_snapshots_deleted(&criteria),
      | SnapshotResponseAction::SnapshotFailure(error) => actor.on_snapshot_failure(&error),
    }
  }
}
