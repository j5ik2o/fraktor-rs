//! Internal typed persistence store actor.

#[cfg(test)]
#[path = "persistence_store_actor_test.rs"]
mod tests;

use alloc::{boxed::Box, format, string::ToString, vec, vec::Vec};

use fraktor_actor_core_kernel_rs::actor::{
  ActorContext,
  error::ActorError,
  messaging::AnyMessageView,
  supervision::{BackoffOnFailureOptions, BackoffSupervisor, BackoffSupervisorStrategy},
};
use fraktor_actor_core_typed_rs::{TypedActorRef, TypedProps};
use fraktor_persistence_core_kernel_rs::{
  error::PersistenceError,
  journal::JournalError,
  persistent::{
    Eventsourced, PersistenceContext, PersistentActor, PersistentRepr, Recovery as KernelRecovery, persistent_props,
  },
  snapshot::{Snapshot, SnapshotError, SnapshotMetadata, SnapshotSelectionCriteria},
};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::{
  EventRejectedError, EventSourcedSignal, PersistenceEffectorConfig, PersistenceId, PublishedEvent,
  internal::{PersistenceStoreCommand, PersistenceStoreReply},
};

type ReplyRef<S, E> = TypedActorRef<PersistenceStoreReply<S, E>>;
type StorePersistHandler<S, E, M> = Box<dyn FnOnce(&mut PersistenceStoreActor<S, E, M>, &PersistentRepr) + Send + Sync>;

pub(crate) struct PersistenceStoreActor<S, E, M>
where
  S: Clone + Send + Sync + 'static,
  E: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static, {
  config:                   PersistenceEffectorConfig<S, E, M>,
  context:                  PersistenceContext<Self>,
  state:                    S,
  recovery_reply_to:        ReplyRef<S, E>,
  pending_persist_reply:    Option<ReplyRef<S, E>>,
  pending_snapshot:         Option<(S, ReplyRef<S, E>)>,
  pending_delete_snapshots: Option<(u64, ReplyRef<S, E>)>,
  pending_delete_events:    Option<(u64, ReplyRef<S, E>)>,
}

impl<S, E, M> PersistenceStoreActor<S, E, M>
where
  S: Clone + Send + Sync + 'static,
  E: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  pub(crate) fn props(
    config: PersistenceEffectorConfig<S, E, M>,
    recovery_reply_to: ReplyRef<S, E>,
  ) -> TypedProps<PersistenceStoreCommand<S, E>> {
    let backoff_config = config.backoff_config().clone();
    let stash_capacity = config.stash_capacity();
    let child_props = persistent_props(move || Self::new(config.clone(), recovery_reply_to.clone()));
    let strategy = BackoffSupervisorStrategy::new(
      backoff_config.min_backoff(),
      backoff_config.max_backoff(),
      backoff_config.random_factor(),
    )
    .with_stash_capacity(stash_capacity);
    let options = BackoffOnFailureOptions::new(child_props, "store".to_string(), strategy);
    TypedProps::from_props(BackoffSupervisor::props_on_failure(options))
  }

  fn new(config: PersistenceEffectorConfig<S, E, M>, recovery_reply_to: ReplyRef<S, E>) -> Self {
    let mut context = PersistenceContext::new(config.persistence_id().as_str().to_string());
    *context.event_adapters_mut() = config.event_adapters().clone();
    Self {
      context,
      state: config.initial_state().clone(),
      config,
      recovery_reply_to,
      pending_persist_reply: None,
      pending_snapshot: None,
      pending_delete_snapshots: None,
      pending_delete_events: None,
    }
  }

  fn persist_event(
    &mut self,
    ctx: &mut ActorContext<'_>,
    event: E,
    reply_to: ReplyRef<S, E>,
  ) -> Result<(), ActorError> {
    self.pending_persist_reply = Some(reply_to.clone());
    let handler = Box::new(move |actor: &mut Self, repr: &PersistentRepr| {
      let Some(persisted_event) = repr.downcast_ref::<E>() else {
        actor.reply_persist_type_mismatch(repr);
        return;
      };
      actor.state = actor.config.apply_event(&actor.state, persisted_event);
      let sequence_nr = actor.context.last_sequence_nr();
      actor.pending_persist_reply = None;
      Self::reply(&reply_to, PersistenceStoreReply::PersistedEvents {
        events: vec![persisted_event.clone()],
        published_events: actor.published_events(repr, persisted_event.clone()),
        sequence_nr,
      });
    });
    self.add_event_to_batch(ctx, event, handler);
    self.flush_batch(ctx)
  }

  fn persist_events(
    &mut self,
    ctx: &mut ActorContext<'_>,
    events: Vec<E>,
    reply_to: ReplyRef<S, E>,
  ) -> Result<(), ActorError> {
    if events.is_empty() {
      Self::reply(&reply_to, PersistenceStoreReply::PersistedEvents {
        events,
        published_events: Vec::new(),
        sequence_nr: self.context.last_sequence_nr(),
      });
      return Ok(());
    }

    let event_count = events.len();
    let persisted_events = ArcShared::new(events.clone());
    let published_events = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new());
    let completion_count = SharedLock::new_with_driver::<DefaultMutex<_>>(0usize);
    self.pending_persist_reply = Some(reply_to.clone());
    for event in events {
      let completion_count = completion_count.clone();
      let persisted_events = persisted_events.clone();
      let published_events = published_events.clone();
      let reply_to = reply_to.clone();
      let handler = Box::new(move |actor: &mut Self, repr: &PersistentRepr| {
        if actor.pending_persist_reply.is_none() {
          return;
        }
        let Some(persisted_event) = repr.downcast_ref::<E>() else {
          actor.reply_persist_type_mismatch(repr);
          return;
        };
        actor.state = actor.config.apply_event(&actor.state, persisted_event);
        if let Some(published_event) = actor.published_event(repr, persisted_event.clone()) {
          published_events.with_lock(|events| {
            events.push(published_event);
          });
        }
        let completed = completion_count.with_lock(|count| {
          *count += 1;
          *count == event_count
        });
        if completed {
          let sequence_nr = actor.context.last_sequence_nr();
          actor.pending_persist_reply = None;
          let mut published_events = published_events.with_lock(|events| events.clone());
          published_events.sort_by_key(PublishedEvent::sequence_nr);
          Self::reply(&reply_to, PersistenceStoreReply::PersistedEvents {
            events: (*persisted_events).clone(),
            published_events,
            sequence_nr,
          });
        }
      });
      self.add_event_to_batch(ctx, event, handler);
    }
    self.flush_batch(ctx)
  }

  fn persist_snapshot(
    &mut self,
    ctx: &mut ActorContext<'_>,
    snapshot: S,
    reply_to: ReplyRef<S, E>,
  ) -> Result<(), ActorError> {
    self.pending_snapshot = Some((snapshot.clone(), reply_to));
    self.save_snapshot(ctx, ArcShared::new(snapshot))
  }

  fn delete_snapshots_to(
    &mut self,
    ctx: &mut ActorContext<'_>,
    to_sequence_nr: u64,
    reply_to: ReplyRef<S, E>,
  ) -> Result<(), ActorError> {
    self.pending_delete_snapshots = Some((to_sequence_nr, reply_to));
    let criteria = SnapshotSelectionCriteria::new(to_sequence_nr, u64::MAX, 0, 0);
    PersistentActor::delete_snapshots(self, ctx, criteria)
  }

  fn delete_events_to(
    &mut self,
    ctx: &mut ActorContext<'_>,
    to_sequence_nr: u64,
    reply_to: ReplyRef<S, E>,
  ) -> Result<(), ActorError> {
    self.pending_delete_events = Some((to_sequence_nr, reply_to));
    if let Err(error) = PersistentActor::delete_messages(self, ctx, to_sequence_nr) {
      self.pending_delete_events = None;
      return Err(error);
    }
    Ok(())
  }

  fn reply(reply_to: &ReplyRef<S, E>, reply: PersistenceStoreReply<S, E>) {
    let mut reply_to = reply_to.clone();
    reply_to.tell(reply);
  }

  fn reply_event_sourced(reply_to: &ReplyRef<S, E>, signal: EventSourcedSignal) {
    Self::reply(reply_to, PersistenceStoreReply::EventSourced { signal });
  }

  fn add_event_to_batch(&mut self, ctx: &mut ActorContext<'_>, event: E, handler: StorePersistHandler<S, E, M>) {
    let sender = ctx.sender().map(|sender| sender.pid());
    self.context.add_to_event_batch(event, true, sender, handler);
  }

  fn published_events(&self, repr: &PersistentRepr, event: E) -> Vec<PublishedEvent<E>> {
    self.published_event(repr, event).map_or_else(Vec::new, |published_event| vec![published_event])
  }

  fn published_event(&self, repr: &PersistentRepr, event: E) -> Option<PublishedEvent<E>> {
    if !self.config.event_publishing_enabled() {
      return None;
    }
    let tags = self.config.event_tags(&event);
    Some(PublishedEvent::new(
      PersistenceId::of_unique_id(repr.persistence_id()),
      repr.sequence_nr(),
      event,
      repr.timestamp(),
      tags,
    ))
  }

  fn reply_persist_type_mismatch(&mut self, repr: &PersistentRepr) {
    if let Some(reply_to) = self.pending_persist_reply.take() {
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::JournalPersistFailed {
        error: PersistenceError::StateMachine(format!(
          "persisted event payload type mismatch for persistence id {} sequence number {}",
          repr.persistence_id(),
          repr.sequence_nr()
        )),
      });
    }
  }

  fn stash_if_waiting_for_snapshot_result(&self, ctx: &mut ActorContext<'_>) -> Result<bool, ActorError> {
    if self.pending_snapshot.is_none()
      && self.pending_delete_snapshots.is_none()
      && self.pending_delete_events.is_none()
    {
      return Ok(false);
    }
    ctx.stash_with_limit(self.config.stash_capacity())?;
    Ok(true)
  }
}

impl<S, E, M> Eventsourced for PersistenceStoreActor<S, E, M>
where
  S: Clone + Send + Sync + 'static,
  E: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn recovery(&self) -> KernelRecovery {
    self.config.recovery().to_kernel()
  }

  fn receive_recover(&mut self, repr: &PersistentRepr) {
    if let Some(event) = repr.downcast_ref::<E>() {
      self.state = self.config.apply_event(&self.state, event);
    }
  }

  fn receive_snapshot(&mut self, snapshot: &Snapshot) {
    if let Some(state) = snapshot.downcast_ref::<S>() {
      self.state = state.clone();
    }
  }

  fn receive_command(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if self.stash_if_waiting_for_snapshot_result(ctx)? {
      return Ok(());
    }
    if let Some(command) = message.downcast_ref::<PersistenceStoreCommand<S, E>>() {
      match command {
        | PersistenceStoreCommand::PersistEvent { event, reply_to } => {
          self.persist_event(ctx, event.clone(), reply_to.clone())?;
        },
        | PersistenceStoreCommand::PersistEvents { events, reply_to } => {
          self.persist_events(ctx, events.clone(), reply_to.clone())?;
        },
        | PersistenceStoreCommand::PersistSnapshot { snapshot, reply_to } => {
          self.persist_snapshot(ctx, snapshot.clone(), reply_to.clone())?;
        },
        | PersistenceStoreCommand::DeleteSnapshots { to_sequence_nr, reply_to } => {
          self.delete_snapshots_to(ctx, *to_sequence_nr, reply_to.clone())?;
        },
        | PersistenceStoreCommand::DeleteEvents { to_sequence_nr, reply_to } => {
          self.delete_events_to(ctx, *to_sequence_nr, reply_to.clone())?;
        },
      }
    }
    Ok(())
  }

  fn on_recovery_completed(&mut self) {
    Self::reply_event_sourced(&self.recovery_reply_to, EventSourcedSignal::RecoveryCompleted);
    Self::reply(&self.recovery_reply_to, PersistenceStoreReply::RecoveryCompleted {
      state:       self.state.clone(),
      sequence_nr: self.context.last_sequence_nr(),
    });
  }

  fn on_persist_failure(&mut self, cause: &JournalError, _repr: &PersistentRepr) {
    if let Some(reply_to) = self.pending_persist_reply.take() {
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::JournalPersistFailed {
        error: PersistenceError::from(cause.clone()),
      });
    }
  }

  fn persist_failure_error(&self, cause: &JournalError, repr: &PersistentRepr) -> ActorError {
    if self.config.persist_failure_backoff_enabled() {
      let message = format!(
        "persistent store restarted after write failure for persistence id {} sequence number {}: {:?}",
        repr.persistence_id(),
        repr.sequence_nr(),
        cause
      );
      ActorError::recoverable(message)
    } else {
      let message = format!(
        "persistent store stopped after write failure for persistence id {} sequence number {}: {:?}",
        repr.persistence_id(),
        repr.sequence_nr(),
        cause
      );
      ActorError::fatal(message)
    }
  }

  fn on_persist_rejected(&mut self, cause: &JournalError, repr: &PersistentRepr) {
    if let Some(reply_to) = self.pending_persist_reply.take() {
      let error = EventRejectedError::new(
        PersistenceId::of_unique_id(repr.persistence_id()),
        repr.sequence_nr(),
        PersistenceError::from(cause.clone()),
      );
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::JournalPersistRejected { error });
    }
  }

  fn on_recovery_failure(&mut self, cause: &PersistenceError) {
    Self::reply_event_sourced(&self.recovery_reply_to, EventSourcedSignal::RecoveryFailed { error: cause.clone() });
  }

  fn on_snapshot_failure(&mut self, cause: &SnapshotError) {
    if let Some((_snapshot, reply_to)) = self.pending_snapshot.take() {
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::SnapshotFailed {
        metadata: Some(self.snapshot_metadata()),
        error:    PersistenceError::from(cause.clone()),
      });
      return;
    }
    if let Some((to_sequence_nr, reply_to)) = self.pending_delete_snapshots.take() {
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::DeleteSnapshotsFailed {
        criteria: Self::snapshot_deletion_criteria(to_sequence_nr),
        error:    PersistenceError::from(cause.clone()),
      });
    }
  }

  fn on_snapshot_saved(&mut self, metadata: &SnapshotMetadata) {
    if let Some((snapshot, reply_to)) = self.pending_snapshot.take() {
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::SnapshotCompleted { metadata: metadata.clone() });
      Self::reply(&reply_to, PersistenceStoreReply::PersistedSnapshot {
        snapshot,
        sequence_nr: self.context.last_sequence_nr(),
      });
    }
  }

  fn on_snapshots_deleted(&mut self, criteria: &SnapshotSelectionCriteria) {
    if let Some((to_sequence_nr, reply_to)) = self.pending_delete_snapshots.take() {
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::DeleteSnapshotsCompleted { criteria: criteria.clone() });
      Self::reply(&reply_to, PersistenceStoreReply::DeletedSnapshots { to_sequence_nr });
    }
  }

  fn on_events_deleted(&mut self, to_sequence_nr: u64) {
    if let Some((_pending_to_sequence_nr, reply_to)) = self.pending_delete_events.take() {
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::DeleteEventsCompleted { to_sequence_nr });
    }
  }

  fn on_events_delete_failure(&mut self, cause: &JournalError, to_sequence_nr: u64) {
    if let Some((_pending_to_sequence_nr, reply_to)) = self.pending_delete_events.take() {
      Self::reply_event_sourced(&reply_to, EventSourcedSignal::DeleteEventsFailed {
        to_sequence_nr,
        error: PersistenceError::from(cause.clone()),
      });
    }
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl<S, E, M> PersistenceStoreActor<S, E, M>
where
  S: Clone + Send + Sync + 'static,
  E: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  fn snapshot_metadata(&self) -> SnapshotMetadata {
    SnapshotMetadata::new(self.context.persistence_id().to_string(), self.context.current_sequence_nr(), 0)
  }

  const fn snapshot_deletion_criteria(to_sequence_nr: u64) -> SnapshotSelectionCriteria {
    SnapshotSelectionCriteria::new(to_sequence_nr, u64::MAX, 0, 0)
  }
}

impl<S, E, M> PersistentActor for PersistenceStoreActor<S, E, M>
where
  S: Clone + Send + Sync + 'static,
  E: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self> {
    &mut self.context
  }

  fn stash_capacity(&self) -> usize {
    self.config.stash_capacity()
  }
}
