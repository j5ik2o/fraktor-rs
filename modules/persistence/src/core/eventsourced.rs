//! Eventsourced actor trait.

#[cfg(test)]
mod tests;

use fraktor_actor_rs::core::{
  actor::{ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  journal_error::JournalError, persistence_error::PersistenceError, persistent_repr::PersistentRepr,
  recovery::Recovery, snapshot::Snapshot, snapshot_error::SnapshotError,
};

/// Event-sourced actor interface.
pub trait Eventsourced<TB: RuntimeToolbox + 'static>: Send {
  /// Returns the persistence id.
  fn persistence_id(&self) -> &str;

  /// Returns the journal actor reference.
  fn journal_actor_ref(&self) -> &ActorRefGeneric<TB>;

  /// Returns the snapshot actor reference.
  fn snapshot_actor_ref(&self) -> &ActorRefGeneric<TB>;

  /// Returns the recovery configuration.
  fn recovery(&self) -> Recovery {
    Recovery::default()
  }

  /// Handles replayed events during recovery.
  fn receive_recover(&mut self, event: &PersistentRepr);

  /// Handles loaded snapshot during recovery.
  fn receive_snapshot(&mut self, snapshot: &Snapshot);

  /// Handles incoming commands.
  ///
  /// # Errors
  ///
  /// Returns `ActorError` when the command cannot be processed.
  fn receive_command(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError>;

  /// Called when recovery completes.
  fn on_recovery_completed(&mut self) {}

  /// Called when persisting fails.
  fn on_persist_failure(&mut self, _cause: &JournalError, _repr: &PersistentRepr) {}

  /// Called when persisting is rejected.
  fn on_persist_rejected(&mut self, _cause: &JournalError, _repr: &PersistentRepr) {}

  /// Called when recovery fails.
  fn on_recovery_failure(&mut self, _cause: &PersistenceError) {}

  /// Called when snapshot operations fail.
  fn on_snapshot_failure(&mut self, _cause: &SnapshotError) {}

  /// Returns the current sequence number.
  fn last_sequence_nr(&self) -> u64;
}
