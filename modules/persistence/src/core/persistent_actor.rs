//! Persistent actor integration with the core actor runtime.

use alloc::{format, string::ToString, vec::Vec};
use core::any::Any;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::{ArcShared, SharedAccess},
  time::MonotonicClock,
};

use crate::core::{
  journal_error::JournalError, persistence_extension::PersistenceExtensionGeneric,
  persistence_extension_id::PersistenceExtensionId, persistent_actor_state::PersistentActorState,
  persistent_recovery::Recovery, persistent_repr::PersistentRepr, snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria, snapshot_store_error::SnapshotStoreError,
};

/// Trait implemented by persistent actors.
pub trait PersistentActor<TB: RuntimeToolbox = NoStdToolbox>: Send {
  /// Returns the persistence id for this actor.
  fn persistence_id(&self) -> &str;

  /// Returns the mutable persistence state owned by the actor.
  fn persistent_state(&mut self) -> &mut PersistentActorState;

  /// Handles a recovered event during replay.
  ///
  /// # Errors
  ///
  /// Returns an error when recovery handling fails.
  fn receive_recover(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    event: &PersistentRepr,
  ) -> Result<(), ActorError>;

  /// Handles a command message after recovery.
  ///
  /// # Errors
  ///
  /// Returns an error when command handling fails.
  fn receive_command(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError>;

  /// Handles a snapshot offer during recovery.
  ///
  /// # Errors
  ///
  /// Returns an error when snapshot handling fails.
  fn receive_snapshot(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _metadata: &SnapshotMetadata,
    _snapshot: &(dyn Any + Send + Sync),
  ) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called after recovery completes.
  ///
  /// # Errors
  ///
  /// Returns an error when post-recovery initialization fails.
  fn on_recovery_completed(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called after recovery and before processing normal messages.
  ///
  /// # Errors
  ///
  /// Returns an error when startup handling fails.
  fn on_start(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Returns recovery configuration.
  fn recovery(&self) -> Recovery {
    Recovery::default()
  }

  /// Persists a single event.
  ///
  /// # Errors
  ///
  /// Returns an error when journal write fails or the extension is not configured.
  fn persist<E: Any + Send + Sync + 'static>(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    event: E,
  ) -> Result<PersistentRepr, ActorError> {
    let persistence_id = self.persistence_id().to_string();
    let sequence_nr = self.persistent_state().sequence_nr().saturating_add(1);
    let writer_id = ctx.pid().to_string();
    let timestamp = now_ticks(ctx);
    let repr = PersistentRepr::from_payload(event, persistence_id, sequence_nr, timestamp, writer_id);
    let extension = persistence_extension_for(ctx)?;
    extension.write_messages(core::slice::from_ref(&repr)).map_err(|error| map_journal_error(&error))?;
    self.persistent_state().set_sequence_nr(sequence_nr);
    Ok(repr)
  }

  /// Persists multiple events in sequence.
  ///
  /// # Errors
  ///
  /// Returns an error when journal write fails or the extension is not configured.
  fn persist_all<E: Any + Send + Sync + 'static, I: IntoIterator<Item = E>>(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    events: I,
  ) -> Result<Vec<PersistentRepr>, ActorError> {
    let persistence_id = self.persistence_id().to_string();
    let writer_id = ctx.pid().to_string();
    let timestamp = now_ticks(ctx);
    let mut sequence_nr = self.persistent_state().sequence_nr();
    let mut reprs = Vec::new();
    for event in events {
      sequence_nr = sequence_nr.saturating_add(1);
      reprs.push(PersistentRepr::from_payload(
        event,
        persistence_id.clone(),
        sequence_nr,
        timestamp,
        writer_id.clone(),
      ));
    }
    if reprs.is_empty() {
      return Ok(reprs);
    }
    let extension = persistence_extension_for(ctx)?;
    extension.write_messages(&reprs).map_err(|error| map_journal_error(&error))?;
    self.persistent_state().set_sequence_nr(sequence_nr);
    Ok(reprs)
  }

  /// Saves a snapshot at the current sequence number.
  ///
  /// # Errors
  ///
  /// Returns an error when snapshot storage fails or the extension is not configured.
  fn save_snapshot<S: Any + Send + Sync + 'static>(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    snapshot: S,
  ) -> Result<SnapshotMetadata, ActorError> {
    let metadata =
      SnapshotMetadata::new(self.persistence_id().to_string(), self.persistent_state().sequence_nr(), now_ticks(ctx));
    let extension = persistence_extension_for(ctx)?;
    extension.save_snapshot(metadata.clone(), ArcShared::new(snapshot)).map_err(|error| map_snapshot_error(&error))?;
    self.persistent_state().set_last_snapshot(metadata.clone());
    Ok(metadata)
  }

  /// Deletes a specific snapshot.
  ///
  /// # Errors
  ///
  /// Returns an error when deletion fails.
  fn delete_snapshot(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    metadata: &SnapshotMetadata,
  ) -> Result<(), ActorError> {
    let extension = persistence_extension_for(ctx)?;
    extension.delete_snapshot(metadata).map_err(|error| map_snapshot_error(&error))
  }

  /// Deletes snapshots matching criteria.
  ///
  /// # Errors
  ///
  /// Returns an error when deletion fails.
  fn delete_snapshots(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    criteria: SnapshotSelectionCriteria,
  ) -> Result<(), ActorError> {
    let extension = persistence_extension_for(ctx)?;
    extension.delete_snapshots(self.persistence_id(), criteria).map_err(|error| map_snapshot_error(&error))
  }
}

/// Wraps a persistent actor implementation into a runtime actor.
#[must_use]
pub fn persistent_actor<TB, A>(actor: A) -> impl Actor<TB>
where
  TB: RuntimeToolbox + 'static,
  A: PersistentActor<TB>, {
  PersistentActorAdapter::new(actor)
}

struct PersistentActorAdapter<A> {
  actor: A,
}

impl<A> PersistentActorAdapter<A> {
  const fn new(actor: A) -> Self {
    Self { actor }
  }
}

impl<TB, A> Actor<TB> for PersistentActorAdapter<A>
where
  TB: RuntimeToolbox + 'static,
  A: PersistentActor<TB>,
{
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    perform_recovery(&mut self.actor, ctx)?;
    self.actor.on_start(ctx)
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    self.actor.receive_command(ctx, message)
  }
}

fn perform_recovery<TB, A>(actor: &mut A, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError>
where
  TB: RuntimeToolbox + 'static,
  A: PersistentActor<TB>, {
  let recovery = actor.recovery();
  let persistence_id = actor.persistence_id().to_string();
  let extension = persistence_extension_for(ctx)?;

  if recovery.is_disabled() {
    let highest = extension.highest_sequence_nr(&persistence_id).map_err(|error| map_journal_error(&error))?;
    actor.persistent_state().set_sequence_nr(highest);
    actor.on_recovery_completed(ctx)?;
    return Ok(());
  }

  let mut from_sequence_nr = 1;
  if !recovery.from_snapshot().is_none() {
    if let Some((metadata, snapshot)) = extension
      .load_snapshot(&persistence_id, recovery.from_snapshot(), recovery.to_sequence_nr())
      .map_err(|error| map_snapshot_error(&error))?
    {
      actor.receive_snapshot(ctx, &metadata, &*snapshot)?;
      actor.persistent_state().set_sequence_nr(metadata.sequence_nr());
      actor.persistent_state().set_last_snapshot(metadata);
    }
    from_sequence_nr = actor.persistent_state().sequence_nr().saturating_add(1);
  }

  let (events, highest) = extension
    .replay_messages(&persistence_id, from_sequence_nr, recovery.to_sequence_nr(), recovery.replay_max())
    .map_err(|error| map_journal_error(&error))?;
  for event in &events {
    actor.receive_recover(ctx, event)?;
    actor.persistent_state().set_sequence_nr(event.sequence_nr());
  }
  if highest > actor.persistent_state().sequence_nr() {
    actor.persistent_state().set_sequence_nr(highest);
  }
  actor.on_recovery_completed(ctx)?;
  Ok(())
}

fn persistence_extension_for<TB: RuntimeToolbox + 'static>(
  ctx: &ActorContextGeneric<'_, TB>,
) -> Result<ArcShared<PersistenceExtensionGeneric<TB>>, ActorError> {
  let id = PersistenceExtensionId::default();
  ctx.system().extended().extension(&id).ok_or_else(|| ActorError::fatal("persistence extension is not registered"))
}

fn map_journal_error(error: &JournalError) -> ActorError {
  ActorError::recoverable(format!("journal error: {error}"))
}

fn map_snapshot_error(error: &SnapshotStoreError) -> ActorError {
  ActorError::recoverable(format!("snapshot store error: {error}"))
}

fn now_ticks<TB: RuntimeToolbox + 'static>(ctx: &ActorContextGeneric<'_, TB>) -> u64 {
  let scheduler = ctx.system().scheduler();
  scheduler.with_read(|scheduler| scheduler.toolbox().clock().now().ticks())
}
