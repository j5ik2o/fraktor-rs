//! Eventsourced actor trait.

#[cfg(test)]
#[path = "eventsourced_test.rs"]
mod tests;

use core::time::Duration;

use fraktor_actor_core_kernel_rs::actor::{ActorContext, error::ActorError, messaging::AnyMessageView};

use crate::{
  error::PersistenceError,
  journal::JournalError,
  persistent::{PersistentRepr, Recovery, RecoveryTimedOut},
  snapshot::{Snapshot, SnapshotError, SnapshotMetadata, SnapshotOffer, SnapshotSelectionCriteria},
};

/// Event-sourced actor interface.
pub trait Eventsourced: Send {
  /// Returns the persistence id.
  fn persistence_id(&self) -> &str;

  /// Returns the recovery configuration.
  fn recovery(&self) -> Recovery {
    Recovery::default()
  }

  /// Returns the timeout used to monitor recovery progress.
  #[must_use]
  fn recovery_event_timeout(&self) -> Duration {
    Duration::from_secs(30)
  }

  /// Handles replayed events during recovery.
  fn receive_recover(&mut self, event: &PersistentRepr);

  /// Handles loaded snapshot during recovery.
  fn receive_snapshot(&mut self, snapshot: &Snapshot);

  /// Handles a loaded snapshot offer during recovery.
  fn receive_snapshot_offer(&mut self, offer: &SnapshotOffer) {
    self.receive_snapshot(offer.snapshot());
  }

  /// Handles incoming commands.
  ///
  /// # Errors
  ///
  /// Returns `ActorError` when the command cannot be processed.
  fn receive_command(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError>;

  /// Called when recovery completes.
  fn on_recovery_completed(&mut self) {}

  /// Called when recovery timed out.
  fn on_recovery_timed_out(&mut self, _signal: &RecoveryTimedOut) {}

  /// Called when persisting fails.
  fn on_persist_failure(&mut self, _cause: &JournalError, _repr: &PersistentRepr) {}

  /// Returns the actor failure raised after a persist failure callback.
  #[must_use]
  fn persist_failure_error(&self, cause: &JournalError, repr: &PersistentRepr) -> ActorError {
    ActorError::fatal(alloc::format!(
      "persistent actor stopped after write failure for persistence id {} sequence number {}: {:?}",
      repr.persistence_id(),
      repr.sequence_nr(),
      cause
    ))
  }

  /// Called when persisting is rejected.
  fn on_persist_rejected(&mut self, _cause: &JournalError, _repr: &PersistentRepr) {}

  /// Called when recovery fails.
  fn on_recovery_failure(&mut self, _cause: &PersistenceError) {}

  /// Called when snapshot operations fail.
  fn on_snapshot_failure(&mut self, _cause: &SnapshotError) {}

  /// Called when snapshot saving succeeds.
  fn on_snapshot_saved(&mut self, _metadata: &SnapshotMetadata) {}

  /// Called when a single snapshot deletion succeeds.
  fn on_snapshot_deleted(&mut self, _metadata: &SnapshotMetadata) {}

  /// Called when criteria-based snapshot deletion succeeds.
  fn on_snapshots_deleted(&mut self, _criteria: &SnapshotSelectionCriteria) {}

  /// Called when event deletion succeeds.
  fn on_events_deleted(&mut self, _to_sequence_nr: u64) {}

  /// Called when event deletion fails.
  fn on_events_delete_failure(&mut self, _cause: &JournalError, _to_sequence_nr: u64) {}

  /// Returns the current sequence number.
  fn last_sequence_nr(&self) -> u64;
}
