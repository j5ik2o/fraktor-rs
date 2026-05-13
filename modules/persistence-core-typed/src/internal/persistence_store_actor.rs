//! Internal typed persistence store actor.

use alloc::{string::ToString, vec, vec::Vec};

use fraktor_actor_core_kernel_rs::actor::{
  ActorContext,
  error::ActorError,
  messaging::AnyMessageView,
  supervision::{BackoffOnFailureOptions, BackoffSupervisor, BackoffSupervisorStrategy},
};
use fraktor_actor_core_typed_rs::{TypedActorRef, TypedProps};
use fraktor_persistence_core_kernel_rs::{
  Eventsourced, JournalError, PersistenceContext, PersistenceError, PersistentActor, PersistentRepr, Snapshot,
  SnapshotError, SnapshotMetadata, SnapshotSelectionCriteria, persistent_props,
};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::{
  PersistenceEffectorConfig,
  internal::{PersistenceStoreCommand, PersistenceStoreReply},
};

type ReplyRef<S, E> = TypedActorRef<PersistenceStoreReply<S, E>>;

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
    let child_config = config.clone();
    let child_reply_to = recovery_reply_to.clone();
    let child_props = persistent_props(move || Self::new(child_config.clone(), child_reply_to.clone()));
    let backoff_config = config.backoff_config();
    let strategy = BackoffSupervisorStrategy::new(
      backoff_config.min_backoff(),
      backoff_config.max_backoff(),
      backoff_config.random_factor(),
    )
    .with_stash_capacity(config.stash_capacity());
    let options = BackoffOnFailureOptions::new(child_props, "store".to_string(), strategy);
    TypedProps::from_props(BackoffSupervisor::props_on_failure(options))
  }

  fn new(config: PersistenceEffectorConfig<S, E, M>, recovery_reply_to: ReplyRef<S, E>) -> Self {
    Self {
      context: PersistenceContext::new(config.persistence_id().as_str().to_string()),
      state: config.initial_state().clone(),
      config,
      recovery_reply_to,
      pending_persist_reply: None,
      pending_snapshot: None,
      pending_delete_snapshots: None,
    }
  }

  fn persist_event(
    &mut self,
    ctx: &mut ActorContext<'_>,
    event: E,
    reply_to: ReplyRef<S, E>,
  ) -> Result<(), ActorError> {
    self.pending_persist_reply = Some(reply_to.clone());
    self.persist(ctx, event, move |actor, persisted_event| {
      actor.state = actor.config.apply_event(&actor.state, persisted_event);
      let sequence_nr = actor.context.last_sequence_nr();
      actor.pending_persist_reply = None;
      Self::reply(&reply_to, PersistenceStoreReply::PersistedEvents {
        events: vec![persisted_event.clone()],
        sequence_nr,
      });
    });
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
        sequence_nr: self.context.last_sequence_nr(),
      });
      return Ok(());
    }

    let event_count = events.len();
    let persisted_events = events.clone();
    let completion_count = SharedLock::new_with_driver::<DefaultMutex<_>>(0usize);
    self.pending_persist_reply = Some(reply_to.clone());
    self.persist_all(ctx, events, move |actor, persisted_event| {
      actor.state = actor.config.apply_event(&actor.state, persisted_event);
      let completed = completion_count.with_lock(|count| {
        *count += 1;
        *count == event_count
      });
      if completed {
        let sequence_nr = actor.context.last_sequence_nr();
        actor.pending_persist_reply = None;
        Self::reply(&reply_to, PersistenceStoreReply::PersistedEvents {
          events: persisted_events.clone(),
          sequence_nr,
        });
      }
    });
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

  fn reply(reply_to: &ReplyRef<S, E>, reply: PersistenceStoreReply<S, E>) {
    let mut reply_to = reply_to.clone();
    reply_to.tell(reply);
  }

  fn reply_failed(reply_to: &ReplyRef<S, E>, error: PersistenceError) {
    Self::reply(reply_to, PersistenceStoreReply::Failed { error });
  }

  fn stash_if_waiting_for_snapshot_result(&self, ctx: &mut ActorContext<'_>) -> Result<bool, ActorError> {
    if self.pending_snapshot.is_none() && self.pending_delete_snapshots.is_none() {
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
      }
    }
    Ok(())
  }

  fn on_recovery_completed(&mut self) {
    Self::reply(&self.recovery_reply_to, PersistenceStoreReply::RecoveryCompleted {
      state:       self.state.clone(),
      sequence_nr: self.context.last_sequence_nr(),
    });
  }

  fn on_persist_failure(&mut self, cause: &JournalError, _repr: &PersistentRepr) {
    if let Some(reply_to) = self.pending_persist_reply.take() {
      Self::reply_failed(&reply_to, PersistenceError::from(cause.clone()));
    }
  }

  fn on_persist_rejected(&mut self, cause: &JournalError, _repr: &PersistentRepr) {
    if let Some(reply_to) = self.pending_persist_reply.take() {
      Self::reply_failed(&reply_to, PersistenceError::from(cause.clone()));
    }
  }

  fn on_recovery_failure(&mut self, cause: &PersistenceError) {
    Self::reply_failed(&self.recovery_reply_to, cause.clone());
  }

  fn on_snapshot_failure(&mut self, cause: &SnapshotError) {
    if let Some((_snapshot, reply_to)) = self.pending_snapshot.take() {
      Self::reply_failed(&reply_to, PersistenceError::from(cause.clone()));
      return;
    }
    if let Some((_to_sequence_nr, reply_to)) = self.pending_delete_snapshots.take() {
      Self::reply_failed(&reply_to, PersistenceError::from(cause.clone()));
    }
  }

  fn on_snapshot_saved(&mut self, _metadata: &SnapshotMetadata) {
    if let Some((snapshot, reply_to)) = self.pending_snapshot.take() {
      Self::reply(&reply_to, PersistenceStoreReply::PersistedSnapshot {
        snapshot,
        sequence_nr: self.context.last_sequence_nr(),
      });
    }
  }

  fn on_snapshots_deleted(&mut self, _criteria: &SnapshotSelectionCriteria) {
    if let Some((to_sequence_nr, reply_to)) = self.pending_delete_snapshots.take() {
      Self::reply(&reply_to, PersistenceStoreReply::DeletedSnapshots { to_sequence_nr });
    }
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
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
