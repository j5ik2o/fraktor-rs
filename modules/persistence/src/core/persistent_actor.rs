//! Persistent actor trait.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::ToString, vec::Vec};
use core::any::Any;

use fraktor_actor_rs::core::{actor::ActorContextGeneric, error::ActorError, messaging::AnyMessageViewGeneric};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  eventsourced::Eventsourced, journal_message::JournalMessage, journal_response::JournalResponse,
  persistence_context::PersistenceContext, persistent_repr::PersistentRepr, snapshot_message::SnapshotMessage,
  snapshot_response::SnapshotResponse,
};

/// Persistent actor interface.
pub trait PersistentActor<TB: RuntimeToolbox + 'static>: Eventsourced<TB> + Sized
where
  Self: 'static, {
  /// Returns the mutable persistence context.
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB>;

  /// Persists a single event and stashes commands.
  fn persist<E: Any + Send + Sync + 'static>(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    event: E,
    handler: impl FnOnce(&mut Self, &E) + Send + Sync + 'static,
  ) {
    let handler_box = Box::new(move |actor: &mut Self, repr: &PersistentRepr| {
      if let Some(event) = repr.downcast_ref::<E>() {
        handler(actor, event);
      }
    });
    self.persistence_context().add_to_event_batch(event, true, handler_box);
  }

  /// Persists a single event without command stashing (fencing).
  ///
  /// Unlike [`Self::persist`], this method does not stash incoming commands
  /// while the event is being persisted. Named "unfenced" to clarify
  /// that no fencing (command stashing) occurs, avoiding confusion
  /// with Tokio's `async` terminology.
  fn persist_unfenced<E: Any + Send + Sync + 'static>(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    event: E,
    handler: impl FnOnce(&mut Self, &E) + Send + Sync + 'static,
  ) {
    let handler_box = Box::new(move |actor: &mut Self, repr: &PersistentRepr| {
      if let Some(event) = repr.downcast_ref::<E>() {
        handler(actor, event);
      }
    });
    self.persistence_context().add_to_event_batch(event, false, handler_box);
  }

  /// Persists multiple events.
  fn persist_all<E: Any + Send + Sync + Clone + 'static>(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    events: Vec<E>,
    handler: impl FnMut(&mut Self, &E) + Send + Sync + 'static,
  ) {
    let handler_box = Box::new(handler);
    let shared_handler = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(handler_box));

    for event in events {
      let handler_clone = shared_handler.clone();
      let handler_box = Box::new(move |actor: &mut Self, repr: &PersistentRepr| {
        if let Some(event) = repr.downcast_ref::<E>() {
          let mut guard = handler_clone.lock();
          (guard.as_mut())(actor, event);
        }
      });
      self.persistence_context().add_to_event_batch(event, true, handler_box);
    }
  }

  /// Flushes the pending batch to the journal.
  fn flush_batch(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) {
    let sender = ctx.self_ref();
    self.persistence_context().flush_batch(sender);
  }

  /// Saves a snapshot.
  fn save_snapshot(&mut self, ctx: &mut ActorContextGeneric<'_, TB>, snapshot: ArcShared<dyn Any + Send + Sync>) {
    let persistence_id = self.persistence_id().to_string();
    let sequence_nr = self.persistence_context().current_sequence_nr();
    let metadata = crate::core::snapshot_metadata::SnapshotMetadata::new(persistence_id, sequence_nr, 0);
    let message = SnapshotMessage::SaveSnapshot { metadata, snapshot, sender: ctx.self_ref() };
    let _ = self.persistence_context().send_snapshot_message(message);
  }

  /// Deletes messages up to the given sequence number.
  fn delete_messages(&mut self, ctx: &mut ActorContextGeneric<'_, TB>, to_sequence_nr: u64) {
    let persistence_id = self.persistence_id().to_string();
    let message = JournalMessage::DeleteMessagesTo { persistence_id, to_sequence_nr, sender: ctx.self_ref() };
    let _ = self.persistence_context().send_write_messages(message);
  }

  /// Deletes snapshots matching the criteria.
  fn delete_snapshots(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    criteria: crate::core::snapshot_selection_criteria::SnapshotSelectionCriteria,
  ) {
    let persistence_id = self.persistence_id().to_string();
    let message = SnapshotMessage::DeleteSnapshots { persistence_id, criteria, sender: ctx.self_ref() };
    let _ = self.persistence_context().send_snapshot_message(message);
  }

  /// Starts recovery by delegating to the base.
  fn start_recovery(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) {
    let sender = ctx.self_ref();
    let recovery = self.recovery();
    self.persistence_context().start_recovery(recovery, sender);
  }

  /// Handles journal responses by delegating to the base.
  fn handle_journal_response(&mut self, response: &JournalResponse) {
    let action = self.persistence_context().handle_journal_response(response);
    action.apply::<TB>(self);
  }

  /// Handles snapshot responses by delegating to the base.
  fn handle_snapshot_response(&mut self, response: &SnapshotResponse, ctx: &mut ActorContextGeneric<'_, TB>) {
    let sender = ctx.self_ref();
    let action = self.persistence_context().handle_snapshot_response(response, sender);
    action.apply::<TB>(self);
  }

  /// Forwards command handling to the actor.
  ///
  /// # Errors
  ///
  /// Returns `ActorError` when the underlying command handler fails.
  fn handle_command(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    self.receive_command(ctx, message)
  }
}
