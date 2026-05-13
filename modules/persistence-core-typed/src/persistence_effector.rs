//! Persistence effector handle.

#[cfg(test)]
#[path = "persistence_effector_test.rs"]
mod tests;

use alloc::{boxed::Box, format, string::ToString, vec, vec::Vec};
use core::marker::PhantomData;

use fraktor_actor_core_kernel_rs::actor::error::ActorError;
use fraktor_actor_core_typed_rs::{
  Behavior, TypedActorRef, TypedProps,
  actor::TypedActorContext,
  dsl::{Behaviors, StashBuffer},
};
use fraktor_persistence_core_kernel_rs::PersistenceError;
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::{
  PersistenceEffectorConfig, PersistenceEffectorMessageAdapter, PersistenceEffectorSignal, PersistenceMode,
  RetentionCriteria,
  internal::{EphemeralPersistenceStore, PersistenceStoreActor, PersistenceStoreCommand, PersistenceStoreReply},
};

type OnReady<S, E, M> = dyn Fn(S, PersistenceEffector<S, E, M>) -> Result<Behavior<M>, ActorError> + Send + Sync;
type EventCallback<E, M> = Box<dyn FnOnce(&E) -> Result<Behavior<M>, ActorError> + Send>;
type EventsCallback<E, M> = Box<dyn FnOnce(&[E]) -> Result<Behavior<M>, ActorError> + Send>;
type SnapshotCallback<S, M> = Box<dyn FnOnce(&S) -> Result<Behavior<M>, ActorError> + Send>;

/// Starts persistence side effects for a typed aggregate actor.
pub struct PersistenceEffector<S, E, M>
where
  S: Send + Sync + 'static,
  E: Send + Sync + 'static,
  M: Send + Sync + 'static, {
  config:          PersistenceEffectorConfig<S, E, M>,
  store_ref:       Option<TypedActorRef<PersistenceStoreCommand<S, E>>>,
  reply_to:        Option<TypedActorRef<PersistenceStoreReply<S, E>>>,
  ephemeral_store: Option<ArcShared<EphemeralPersistenceStore>>,
  on_ready:        Option<ArcShared<OnReady<S, E, M>>>,
  sequence_nr:     SharedLock<u64>,
  _message:        PhantomData<fn() -> M>,
}

impl<S, E, M> PersistenceEffector<S, E, M>
where
  S: Clone + Send + Sync + 'static,
  E: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  /// Builds typed props that configure a stash-capable mailbox.
  #[must_use]
  pub fn props<F>(config: PersistenceEffectorConfig<S, E, M>, on_ready: F) -> TypedProps<M>
  where
    F: Fn(S, PersistenceEffector<S, E, M>) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    let on_ready = ArcShared::new(on_ready);
    TypedProps::from_behavior_factory(move || Self::from_config_with_shared(config.clone(), on_ready.clone()))
      .with_stash_mailbox()
  }

  /// Builds a behavior from an effector config.
  ///
  /// Callers using this lower-level API must pair the returned behavior with
  /// `TypedProps::with_stash_mailbox()`.
  #[must_use]
  pub fn from_config<F>(config: PersistenceEffectorConfig<S, E, M>, on_ready: F) -> Behavior<M>
  where
    F: Fn(S, PersistenceEffector<S, E, M>) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    Self::from_config_with_shared(config, ArcShared::new(on_ready))
  }

  fn from_config_with_shared(
    config: PersistenceEffectorConfig<S, E, M>,
    on_ready: ArcShared<OnReady<S, E, M>>,
  ) -> Behavior<M> {
    Behaviors::setup_result(move |ctx| {
      let config = config.clone();
      let on_ready = on_ready.clone();
      config.validate().map_err(|error| ActorError::fatal(error.to_string()))?;
      match config.persistence_mode() {
        | PersistenceMode::Deferred => {
          let effector = Self::deferred(config.clone());
          return on_ready(config.initial_state().clone(), effector);
        },
        | PersistenceMode::Ephemeral => {
          let store = EphemeralPersistenceStore::for_system(&ctx.system());
          let (state, sequence_nr) = store.recover(&config).map_err(map_persistence_error)?;
          let effector = Self::ephemeral(config.clone(), store, sequence_nr);
          return on_ready(state, effector);
        },
        | PersistenceMode::Persisted => {},
      }
      let message_adapter = config
        .message_adapter()
        .cloned()
        .ok_or_else(|| ActorError::fatal("persistence message adapter is not configured"))?;
      let reply_to = ctx
        .message_adapter(move |reply: PersistenceStoreReply<S, E>| {
          Ok(message_adapter.wrap_signal(PersistenceEffectorSignal::from(reply)))
        })
        .map_err(|error| ActorError::fatal(format!("persistence reply adapter registration failed: {error:?}")))?;
      let store_props = PersistenceStoreActor::<S, E, M>::props(config.clone(), reply_to.clone());
      let store_child = ctx
        .spawn_child(&store_props)
        .map_err(|error| ActorError::fatal(format!("persistence store spawn failed: {error:?}")))?;
      Ok(Self::await_recovery(config.clone(), store_child.actor_ref(), reply_to, on_ready.clone()))
    })
  }

  fn deferred(config: PersistenceEffectorConfig<S, E, M>) -> Self {
    Self {
      config,
      store_ref: None,
      reply_to: None,
      ephemeral_store: None,
      on_ready: None,
      sequence_nr: SharedLock::new_with_driver::<DefaultMutex<_>>(0),
      _message: PhantomData,
    }
  }

  fn ephemeral(
    config: PersistenceEffectorConfig<S, E, M>,
    store: ArcShared<EphemeralPersistenceStore>,
    sequence_nr: u64,
  ) -> Self {
    Self {
      config,
      store_ref: None,
      reply_to: None,
      ephemeral_store: Some(store),
      on_ready: None,
      sequence_nr: SharedLock::new_with_driver::<DefaultMutex<_>>(sequence_nr),
      _message: PhantomData,
    }
  }

  fn active(
    config: PersistenceEffectorConfig<S, E, M>,
    store_ref: TypedActorRef<PersistenceStoreCommand<S, E>>,
    reply_to: TypedActorRef<PersistenceStoreReply<S, E>>,
    sequence_nr: u64,
    on_ready: ArcShared<OnReady<S, E, M>>,
  ) -> Self {
    Self {
      config,
      store_ref: Some(store_ref),
      reply_to: Some(reply_to),
      ephemeral_store: None,
      on_ready: Some(on_ready),
      sequence_nr: SharedLock::new_with_driver::<DefaultMutex<_>>(sequence_nr),
      _message: PhantomData,
    }
  }

  fn await_recovery(
    config: PersistenceEffectorConfig<S, E, M>,
    store_ref: TypedActorRef<PersistenceStoreCommand<S, E>>,
    reply_to: TypedActorRef<PersistenceStoreReply<S, E>>,
    on_ready: ArcShared<OnReady<S, E, M>>,
  ) -> Behavior<M> {
    let adapter = match config.message_adapter().cloned() {
      | Some(adapter) => adapter,
      | None => return fatal_behavior("persistence message adapter is not configured"),
    };
    Behaviors::with_stash(config.stash_capacity(), move |stash| {
      let adapter = adapter.clone();
      let config = config.clone();
      let store_ref = store_ref.clone();
      let reply_to = reply_to.clone();
      let on_ready = on_ready.clone();
      Behaviors::receive_message(move |ctx, message| {
        if let Some(signal) = adapter.unwrap_signal(message) {
          return match signal {
            | PersistenceEffectorSignal::RecoveryCompleted { state, sequence_nr } => {
              let effector =
                Self::active(config.clone(), store_ref.clone(), reply_to.clone(), *sequence_nr, on_ready.clone());
              let next = on_ready(state.clone(), effector)?;
              stash.unstash_all(ctx)?;
              Ok(next)
            },
            | PersistenceEffectorSignal::Failed { error } => Err(ActorError::fatal(error.to_string())),
            | _ => Ok(Behaviors::unhandled()),
          };
        }
        stash.stash(ctx)?;
        Ok(Behaviors::same())
      })
    })
  }

  /// Returns the current persistence mode.
  #[must_use]
  pub const fn persistence_mode(&self) -> PersistenceMode {
    self.config.persistence_mode()
  }

  /// Returns the latest known sequence number.
  #[must_use]
  pub fn sequence_nr(&self) -> u64 {
    self.sequence_nr.with_lock(|sequence_nr| *sequence_nr)
  }

  /// Persists one event and runs a one-shot callback after success.
  pub fn persist_event<F>(
    &self,
    _ctx: &mut TypedActorContext<'_, M>,
    event: E,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&E) -> Result<Behavior<M>, ActorError> + Send + 'static, {
    if self.config.persistence_mode() == PersistenceMode::Deferred {
      return on_persisted(&event);
    }
    if self.config.persistence_mode() == PersistenceMode::Ephemeral {
      let (events, sequence_nr) = self.persist_ephemeral_events(vec![event])?;
      self.update_sequence_nr(sequence_nr);
      return match events.first() {
        | Some(event) => on_persisted(event),
        | None => Ok(Behaviors::same()),
      };
    }
    let mut store_ref = self.store_ref()?;
    let reply_to = self.reply_to()?;
    store_ref
      .try_tell(PersistenceStoreCommand::PersistEvent { event, reply_to })
      .map_err(|error| ActorError::fatal(format!("persist event send failed: {error:?}")))?;
    Ok(self.wait_for_event(Box::new(on_persisted)))
  }

  /// Persists multiple events as one batch and runs a one-shot callback after success.
  pub fn persist_events<F>(
    &self,
    _ctx: &mut TypedActorContext<'_, M>,
    events: Vec<E>,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&[E]) -> Result<Behavior<M>, ActorError> + Send + 'static, {
    if self.config.persistence_mode() == PersistenceMode::Deferred {
      return on_persisted(events.as_slice());
    }
    if self.config.persistence_mode() == PersistenceMode::Ephemeral {
      let (events, sequence_nr) = self.persist_ephemeral_events(events)?;
      self.update_sequence_nr(sequence_nr);
      return on_persisted(events.as_slice());
    }
    let mut store_ref = self.store_ref()?;
    let reply_to = self.reply_to()?;
    store_ref
      .try_tell(PersistenceStoreCommand::PersistEvents { events, reply_to })
      .map_err(|error| ActorError::fatal(format!("persist events send failed: {error:?}")))?;
    Ok(self.wait_for_events(Box::new(on_persisted)))
  }

  /// Persists one snapshot and runs a one-shot callback after success.
  pub fn persist_snapshot<F>(
    &self,
    _ctx: &mut TypedActorContext<'_, M>,
    snapshot: S,
    force: bool,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&S) -> Result<Behavior<M>, ActorError> + Send + 'static, {
    if self.config.persistence_mode() == PersistenceMode::Deferred {
      return on_persisted(&snapshot);
    }
    let sequence_nr = self.sequence_nr();
    if !self.should_save_snapshot(None, &snapshot, sequence_nr, force) {
      return on_persisted(&snapshot);
    }
    if self.config.persistence_mode() == PersistenceMode::Ephemeral {
      let snapshot = self.persist_ephemeral_snapshot(snapshot, sequence_nr)?;
      return on_persisted(&snapshot);
    }
    let mut store_ref = self.store_ref()?;
    let reply_to = self.reply_to()?;
    store_ref
      .try_tell(PersistenceStoreCommand::PersistSnapshot { snapshot, reply_to })
      .map_err(|error| ActorError::fatal(format!("persist snapshot send failed: {error:?}")))?;
    Ok(self.wait_for_snapshot(Box::new(on_persisted)))
  }

  /// Persists one event and evaluates snapshot criteria with the supplied snapshot.
  pub fn persist_event_with_snapshot<F>(
    &self,
    _ctx: &mut TypedActorContext<'_, M>,
    event: E,
    snapshot: S,
    force_snapshot: bool,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&E) -> Result<Behavior<M>, ActorError> + Send + 'static, {
    if self.config.persistence_mode() == PersistenceMode::Deferred {
      return on_persisted(&event);
    }
    if self.config.persistence_mode() == PersistenceMode::Ephemeral {
      let (events, sequence_nr) = self.persist_ephemeral_events(vec![event])?;
      self.update_sequence_nr(sequence_nr);
      if self.should_save_snapshot(events.last(), &snapshot, sequence_nr, force_snapshot) {
        self.persist_ephemeral_snapshot(snapshot, sequence_nr)?;
      }
      return match events.first() {
        | Some(event) => on_persisted(event),
        | None => Ok(Behaviors::same()),
      };
    }
    let mut store_ref = self.store_ref()?;
    let reply_to = self.reply_to()?;
    store_ref
      .try_tell(PersistenceStoreCommand::PersistEvent { event, reply_to })
      .map_err(|error| ActorError::fatal(format!("persist event send failed: {error:?}")))?;
    Ok(self.wait_for_events_then_snapshot(
      Box::new(move |events| match events.first() {
        | Some(event) => on_persisted(event),
        | None => Ok(Behaviors::same()),
      }),
      snapshot,
      force_snapshot,
    ))
  }

  /// Persists event batch and evaluates snapshot criteria with the supplied snapshot.
  pub fn persist_events_with_snapshot<F>(
    &self,
    _ctx: &mut TypedActorContext<'_, M>,
    events: Vec<E>,
    snapshot: S,
    force_snapshot: bool,
    on_persisted: F,
  ) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&[E]) -> Result<Behavior<M>, ActorError> + Send + 'static, {
    if self.config.persistence_mode() == PersistenceMode::Deferred {
      return on_persisted(events.as_slice());
    }
    if self.config.persistence_mode() == PersistenceMode::Ephemeral {
      let (events, sequence_nr) = self.persist_ephemeral_events(events)?;
      self.update_sequence_nr(sequence_nr);
      if self.should_save_snapshot(events.last(), &snapshot, sequence_nr, force_snapshot) {
        self.persist_ephemeral_snapshot(snapshot, sequence_nr)?;
      }
      return on_persisted(events.as_slice());
    }
    let mut store_ref = self.store_ref()?;
    let reply_to = self.reply_to()?;
    store_ref
      .try_tell(PersistenceStoreCommand::PersistEvents { events, reply_to })
      .map_err(|error| ActorError::fatal(format!("persist events send failed: {error:?}")))?;
    Ok(self.wait_for_events_then_snapshot(Box::new(on_persisted), snapshot, force_snapshot))
  }

  fn store_ref(&self) -> Result<TypedActorRef<PersistenceStoreCommand<S, E>>, ActorError> {
    self.store_ref.clone().ok_or_else(|| ActorError::fatal("persistence store is not available"))
  }

  fn reply_to(&self) -> Result<TypedActorRef<PersistenceStoreReply<S, E>>, ActorError> {
    self.reply_to.clone().ok_or_else(|| ActorError::fatal("persistence reply adapter is not available"))
  }

  fn ephemeral_store(&self) -> Result<ArcShared<EphemeralPersistenceStore>, ActorError> {
    self.ephemeral_store.clone().ok_or_else(|| ActorError::fatal("ephemeral persistence store is not available"))
  }

  fn persist_ephemeral_events(&self, events: Vec<E>) -> Result<(Vec<E>, u64), ActorError> {
    self.ephemeral_store()?.persist_events(&self.config, events).map_err(map_persistence_error)
  }

  fn persist_ephemeral_snapshot(&self, snapshot: S, sequence_nr: u64) -> Result<S, ActorError> {
    let store = self.ephemeral_store()?;
    let snapshot = store.persist_snapshot(&self.config, snapshot, sequence_nr).map_err(map_persistence_error)?;
    if let Some(to_sequence_nr) = Self::retention_delete_to(*self.config.retention_criteria(), sequence_nr) {
      store
        .delete_snapshots_to(self.config.persistence_id().as_str(), to_sequence_nr)
        .map_err(map_persistence_error)?;
    }
    Ok(snapshot)
  }

  fn should_save_snapshot(&self, event: Option<&E>, snapshot: &S, sequence_nr: u64, force: bool) -> bool {
    force || self.config.snapshot_criteria().should_take_snapshot(event, snapshot, sequence_nr)
  }

  fn update_sequence_nr(&self, sequence_nr: u64) {
    self.sequence_nr.with_lock(|stored_sequence_nr| {
      *stored_sequence_nr = sequence_nr;
    });
  }

  fn recover_after_store_restart(
    &self,
    ctx: &mut TypedActorContext<'_, M>,
    stash: &StashBuffer<M>,
    state: &S,
    sequence_nr: u64,
  ) -> Result<Behavior<M>, ActorError> {
    let on_ready =
      self.on_ready.clone().ok_or_else(|| ActorError::fatal("persistence recovery callback is not available"))?;
    let effector =
      Self::active(self.config.clone(), self.store_ref()?, self.reply_to()?, sequence_nr, on_ready.clone());
    let next = on_ready(state.clone(), effector)?;
    stash.unstash_all(ctx)?;
    Ok(next)
  }

  fn wait_for_event(&self, callback: EventCallback<E, M>) -> Behavior<M> {
    let callback = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(callback));
    self.wait_for_events(Box::new(move |events| match events.first() {
      | Some(event) => {
        callback.with_lock(|slot| slot.take().map(|callback| callback(event)).unwrap_or_else(|| Ok(Behaviors::same())))
      },
      | None => Ok(Behaviors::same()),
    }))
  }

  fn wait_for_events(&self, callback: EventsCallback<E, M>) -> Behavior<M> {
    let adapter = match self.config.message_adapter().cloned() {
      | Some(adapter) => adapter,
      | None => return fatal_behavior("persistence message adapter is not configured"),
    };
    let callback = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(callback));
    let effector = self.clone();
    let sequence_nr_cell = self.sequence_nr.clone();
    Behaviors::with_stash(self.config.stash_capacity(), move |stash| {
      let adapter = adapter.clone();
      let callback = callback.clone();
      let effector = effector.clone();
      let sequence_nr_cell = sequence_nr_cell.clone();
      Behaviors::receive_message(move |ctx, message| {
        if let Some(signal) = adapter.unwrap_signal(message) {
          return match signal {
            | PersistenceEffectorSignal::PersistedEvents { events, sequence_nr } => {
              sequence_nr_cell.with_lock(|stored_sequence_nr| {
                *stored_sequence_nr = *sequence_nr;
              });
              let next = callback.with_lock(|slot| {
                slot.take().map(|callback| callback(events.as_slice())).unwrap_or_else(|| Ok(Behaviors::same()))
              })?;
              stash.unstash_all(ctx)?;
              Ok(next)
            },
            | PersistenceEffectorSignal::RecoveryCompleted { state, sequence_nr } => {
              effector.recover_after_store_restart(ctx, &stash, state, *sequence_nr)
            },
            | PersistenceEffectorSignal::Failed { error } => Err(ActorError::fatal(error.to_string())),
            | _ => Ok(Behaviors::unhandled()),
          };
        }
        stash.stash(ctx)?;
        Ok(Behaviors::same())
      })
    })
  }

  fn wait_for_events_then_snapshot(
    &self,
    callback: EventsCallback<E, M>,
    snapshot: S,
    force_snapshot: bool,
  ) -> Behavior<M> {
    let adapter = match self.config.message_adapter().cloned() {
      | Some(adapter) => adapter,
      | None => return fatal_behavior("persistence message adapter is not configured"),
    };
    let callback = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(callback));
    let effector = self.clone();
    let sequence_nr_cell = self.sequence_nr.clone();
    let snapshot_criteria = self.config.snapshot_criteria().clone();
    Behaviors::with_stash(self.config.stash_capacity(), move |stash| {
      let adapter = adapter.clone();
      let callback = callback.clone();
      let effector = effector.clone();
      let sequence_nr_cell = sequence_nr_cell.clone();
      let snapshot = snapshot.clone();
      let snapshot_criteria = snapshot_criteria.clone();
      Behaviors::receive_message(move |ctx, message| {
        if let Some(signal) = adapter.unwrap_signal(message) {
          return match signal {
            | PersistenceEffectorSignal::PersistedEvents { events, sequence_nr } => {
              sequence_nr_cell.with_lock(|stored_sequence_nr| {
                *stored_sequence_nr = *sequence_nr;
              });
              let should_snapshot =
                force_snapshot || snapshot_criteria.should_take_snapshot(events.last(), &snapshot, *sequence_nr);
              if should_snapshot {
                let mut store_ref = effector.store_ref()?;
                let reply_to = effector.reply_to()?;
                store_ref
                  .try_tell(PersistenceStoreCommand::PersistSnapshot { snapshot: snapshot.clone(), reply_to })
                  .map_err(|error| ActorError::fatal(format!("persist snapshot send failed: {error:?}")))?;
                let persisted_events = events.clone();
                let callback = callback.with_lock(Option::take);
                stash.unstash_all(ctx)?;
                return Ok(match callback {
                  | Some(callback) => {
                    effector.wait_for_snapshot(Box::new(move |_snapshot| callback(persisted_events.as_slice())))
                  },
                  | None => Behaviors::same(),
                });
              }
              let next = callback.with_lock(|slot| {
                slot.take().map(|callback| callback(events.as_slice())).unwrap_or_else(|| Ok(Behaviors::same()))
              })?;
              stash.unstash_all(ctx)?;
              Ok(next)
            },
            | PersistenceEffectorSignal::RecoveryCompleted { state, sequence_nr } => {
              effector.recover_after_store_restart(ctx, &stash, state, *sequence_nr)
            },
            | PersistenceEffectorSignal::Failed { error } => Err(ActorError::fatal(error.to_string())),
            | _ => Ok(Behaviors::unhandled()),
          };
        }
        stash.stash(ctx)?;
        Ok(Behaviors::same())
      })
    })
  }

  fn wait_for_snapshot(&self, callback: SnapshotCallback<S, M>) -> Behavior<M> {
    let adapter = match self.config.message_adapter().cloned() {
      | Some(adapter) => adapter,
      | None => return fatal_behavior("persistence message adapter is not configured"),
    };
    let callback = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(callback));
    let sequence_nr_cell = self.sequence_nr.clone();
    let store_ref = self.store_ref.clone();
    let reply_to = self.reply_to.clone();
    let retention_criteria = *self.config.retention_criteria();
    let effector = self.clone();
    Behaviors::with_stash(self.config.stash_capacity(), move |stash| {
      let adapter = adapter.clone();
      let callback = callback.clone();
      let sequence_nr_cell = sequence_nr_cell.clone();
      let store_ref = store_ref.clone();
      let reply_to = reply_to.clone();
      let effector = effector.clone();
      Behaviors::receive_message(move |ctx, message| {
        if let Some(signal) = adapter.unwrap_signal(message) {
          return match signal {
            | PersistenceEffectorSignal::PersistedSnapshot { snapshot, sequence_nr } => {
              sequence_nr_cell.with_lock(|stored_sequence_nr| {
                *stored_sequence_nr = *sequence_nr;
              });
              if let Some(to_sequence_nr) = Self::retention_delete_to(retention_criteria, *sequence_nr) {
                let mut store_ref =
                  store_ref.clone().ok_or_else(|| ActorError::fatal("persistence store is not available"))?;
                let reply_to =
                  reply_to.clone().ok_or_else(|| ActorError::fatal("persistence reply adapter is not available"))?;
                store_ref
                  .try_tell(PersistenceStoreCommand::DeleteSnapshots { to_sequence_nr, reply_to })
                  .map_err(|error| ActorError::fatal(format!("delete snapshots send failed: {error:?}")))?;
                return Ok(Self::wait_for_deleted_snapshots(
                  adapter.clone(),
                  callback.clone(),
                  snapshot.clone(),
                  to_sequence_nr,
                  stash,
                  effector.clone(),
                ));
              }
              let next = callback.with_lock(|slot| {
                slot.take().map(|callback| callback(snapshot)).unwrap_or_else(|| Ok(Behaviors::same()))
              })?;
              stash.unstash_all(ctx)?;
              Ok(next)
            },
            | PersistenceEffectorSignal::RecoveryCompleted { state, sequence_nr } => {
              effector.recover_after_store_restart(ctx, &stash, state, *sequence_nr)
            },
            | PersistenceEffectorSignal::Failed { error } => Err(ActorError::fatal(error.to_string())),
            | _ => Ok(Behaviors::unhandled()),
          };
        }
        stash.stash(ctx)?;
        Ok(Behaviors::same())
      })
    })
  }

  fn wait_for_deleted_snapshots(
    adapter: PersistenceEffectorMessageAdapter<S, E, M>,
    callback: SharedLock<Option<SnapshotCallback<S, M>>>,
    snapshot: S,
    to_sequence_nr: u64,
    stash: StashBuffer<M>,
    effector: PersistenceEffector<S, E, M>,
  ) -> Behavior<M> {
    let snapshot = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(snapshot));
    Behaviors::receive_message(move |ctx, message| {
      if let Some(signal) = adapter.unwrap_signal(message) {
        return match signal {
          | PersistenceEffectorSignal::DeletedSnapshots { to_sequence_nr: deleted_to_sequence_nr }
            if *deleted_to_sequence_nr == to_sequence_nr =>
          {
            let next = snapshot.with_lock(|snapshot_slot| {
              callback.with_lock(|callback_slot| match (snapshot_slot.take(), callback_slot.take()) {
                | (Some(snapshot), Some(callback)) => callback(&snapshot),
                | _ => Ok(Behaviors::same()),
              })
            })?;
            stash.unstash_all(ctx)?;
            Ok(next)
          },
          | PersistenceEffectorSignal::DeletedSnapshots { .. } => {
            Err(ActorError::fatal("unexpected snapshot deletion acknowledgement"))
          },
          | PersistenceEffectorSignal::RecoveryCompleted { state, sequence_nr } => {
            effector.recover_after_store_restart(ctx, &stash, state, *sequence_nr)
          },
          | PersistenceEffectorSignal::Failed { error } => Err(ActorError::fatal(error.to_string())),
          | _ => Ok(Behaviors::unhandled()),
        };
      }
      stash.stash(ctx)?;
      Ok(Behaviors::same())
    })
  }

  fn retention_delete_to(retention_criteria: RetentionCriteria, sequence_nr: u64) -> Option<u64> {
    let snapshot_every = retention_criteria.snapshot_every_interval()?;
    let keep_snapshots = retention_criteria.keep_snapshots()?;
    if snapshot_every == 0 || keep_snapshots == 0 {
      return None;
    }
    let latest_snapshot_sequence_nr = sequence_nr - (sequence_nr % snapshot_every);
    if latest_snapshot_sequence_nr < snapshot_every {
      return None;
    }
    let kept_snapshot_span = snapshot_every.checked_mul(keep_snapshots.saturating_sub(1))?;
    let oldest_kept_snapshot = latest_snapshot_sequence_nr.checked_sub(kept_snapshot_span)?;
    if oldest_kept_snapshot == 0 {
      return None;
    }
    let max_sequence_nr_to_delete = oldest_kept_snapshot.checked_sub(snapshot_every)?;
    (max_sequence_nr_to_delete > 0).then_some(max_sequence_nr_to_delete)
  }
}

impl<S, E, M> Clone for PersistenceEffector<S, E, M>
where
  S: Clone + Send + Sync + 'static,
  E: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self {
      config:          self.config.clone(),
      store_ref:       self.store_ref.clone(),
      reply_to:        self.reply_to.clone(),
      ephemeral_store: self.ephemeral_store.clone(),
      on_ready:        self.on_ready.clone(),
      sequence_nr:     self.sequence_nr.clone(),
      _message:        PhantomData,
    }
  }
}

fn map_persistence_error(error: PersistenceError) -> ActorError {
  ActorError::fatal(error.to_string())
}

fn fatal_behavior<M>(message: &'static str) -> Behavior<M>
where
  M: Send + Sync + 'static, {
  Behaviors::receive_message(move |_ctx, _message| Err(ActorError::fatal(message)))
}
