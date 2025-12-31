//! Actions derived from snapshot responses.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{eventsourced::Eventsourced, snapshot::Snapshot, snapshot_error::SnapshotError};

/// Actions to apply on the actor after snapshot response handling.
pub(crate) enum SnapshotResponseAction {
  /// No actor callback required.
  None,
  /// Deliver a loaded snapshot.
  ReceiveSnapshot(Snapshot),
  /// Notify snapshot failure.
  SnapshotFailure(SnapshotError),
}

impl SnapshotResponseAction {
  pub(crate) fn apply<TB: RuntimeToolbox + 'static>(self, actor: &mut impl Eventsourced<TB>) {
    match self {
      | SnapshotResponseAction::None => {},
      | SnapshotResponseAction::ReceiveSnapshot(snapshot) => actor.receive_snapshot(&snapshot),
      | SnapshotResponseAction::SnapshotFailure(error) => actor.on_snapshot_failure(&error),
    }
  }
}
