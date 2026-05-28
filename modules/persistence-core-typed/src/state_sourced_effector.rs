//! State-sourced effector handle.

#[cfg(test)]
#[path = "state_sourced_effector_test.rs"]
mod tests;

use alloc::{boxed::Box, format, string::ToString};
use core::marker::PhantomData;

use fraktor_actor_core_kernel_rs::actor::error::ActorError;
use fraktor_actor_core_typed_rs::{Behavior, TypedActorRef, TypedProps, dsl::Behaviors};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::{
  StateSourcedEffectorConfig, StateSourcedEffectorSignal,
  internal::{StateSourcedStoreActor, StateSourcedStoreCommand, StateSourcedStoreReply},
};

type OnReady<S, M> = dyn Fn(Option<S>, StateSourcedEffector<S, M>) -> Result<Behavior<M>, ActorError> + Send + Sync;
type PersistStateCallback<S, M> = Box<dyn FnOnce(&S, u64) -> Result<Behavior<M>, ActorError> + Send>;
type DeleteStateCallback<M> = Box<dyn FnOnce(u64) -> Result<Behavior<M>, ActorError> + Send>;

/// Starts durable state side effects for a typed aggregate actor.
pub struct StateSourcedEffector<S, M>
where
  S: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static, {
  config:    StateSourcedEffectorConfig<S, M>,
  store_ref: TypedActorRef<StateSourcedStoreCommand<S>>,
  reply_to:  TypedActorRef<StateSourcedStoreReply<S>>,
  revision:  SharedLock<u64>,
  _message:  PhantomData<fn() -> M>,
}

impl<S, M> StateSourcedEffector<S, M>
where
  S: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  /// Builds typed props that configure a stash-capable mailbox.
  #[must_use]
  pub fn props<F>(config: StateSourcedEffectorConfig<S, M>, on_ready: F) -> TypedProps<M>
  where
    F: Fn(Option<S>, StateSourcedEffector<S, M>) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    let on_ready = ArcShared::new(on_ready);
    TypedProps::from_behavior_factory(move || Self::from_config_with_shared(config.clone(), on_ready.clone()))
      .with_stash_mailbox()
  }

  /// Builds a behavior from a state-sourced effector config.
  ///
  /// Callers using this lower-level API must pair the returned behavior with
  /// `TypedProps::with_stash_mailbox()`.
  #[must_use]
  pub fn from_config<F>(config: StateSourcedEffectorConfig<S, M>, on_ready: F) -> Behavior<M>
  where
    F: Fn(Option<S>, StateSourcedEffector<S, M>) -> Result<Behavior<M>, ActorError> + Send + Sync + 'static, {
    Self::from_config_with_shared(config, ArcShared::new(on_ready))
  }

  fn from_config_with_shared(
    config: StateSourcedEffectorConfig<S, M>,
    on_ready: ArcShared<OnReady<S, M>>,
  ) -> Behavior<M> {
    Behaviors::setup_result(move |ctx| {
      let config = config.clone();
      let on_ready = on_ready.clone();
      config.validate().map_err(|error| ActorError::fatal(error.to_string()))?;
      let message_adapter = config
        .message_adapter()
        .cloned()
        .ok_or_else(|| ActorError::fatal("state-sourced message adapter is not configured"))?;
      let provider = config
        .store_provider()
        .cloned()
        .ok_or_else(|| ActorError::fatal("durable state store provider is not configured"))?;
      let reply_to = ctx
        .message_adapter(move |reply: StateSourcedStoreReply<S>| {
          Ok(message_adapter.wrap_signal(StateSourcedEffectorSignal::from(reply)))
        })
        .map_err(|error| ActorError::fatal(format!("state-sourced reply adapter registration failed: {error:?}")))?;
      let store_props = StateSourcedStoreActor::<S, M>::props(config.clone(), provider);
      let store_child = ctx
        .spawn_child(&store_props)
        .map_err(|error| ActorError::fatal(format!("state-sourced store spawn failed: {error:?}")))?;
      let mut store_ref = store_child.actor_ref();
      store_ref
        .try_tell(StateSourcedStoreCommand::Recover { reply_to: reply_to.clone() })
        .map_err(|error| ActorError::fatal(format!("state-sourced recovery send failed: {error:?}")))?;
      Ok(Self::await_recovery(config, store_ref, reply_to, on_ready))
    })
  }

  fn active(
    config: StateSourcedEffectorConfig<S, M>,
    store_ref: TypedActorRef<StateSourcedStoreCommand<S>>,
    reply_to: TypedActorRef<StateSourcedStoreReply<S>>,
    revision: u64,
  ) -> Self {
    Self {
      config,
      store_ref,
      reply_to,
      revision: SharedLock::new_with_driver::<DefaultMutex<_>>(revision),
      _message: PhantomData,
    }
  }

  fn await_recovery(
    config: StateSourcedEffectorConfig<S, M>,
    store_ref: TypedActorRef<StateSourcedStoreCommand<S>>,
    reply_to: TypedActorRef<StateSourcedStoreReply<S>>,
    on_ready: ArcShared<OnReady<S, M>>,
  ) -> Behavior<M> {
    let adapter = match config.message_adapter().cloned() {
      | Some(adapter) => adapter,
      | None => return fatal_behavior("state-sourced message adapter is not configured"),
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
            | StateSourcedEffectorSignal::RecoveryCompleted { state, revision, .. } => {
              let effector = Self::active(config.clone(), store_ref.clone(), reply_to.clone(), *revision);
              let next = on_ready(state.clone(), effector)?;
              stash.unstash_all(ctx)?;
              Ok(next)
            },
            | StateSourcedEffectorSignal::RecoveryFailed { error, .. } => Err(ActorError::fatal(error.to_string())),
            | _ => Ok(Behaviors::unhandled()),
          };
        }
        stash.stash(ctx)?;
        Ok(Behaviors::same())
      })
    })
  }

  /// Returns the latest known durable state revision.
  #[must_use]
  pub fn revision(&self) -> u64 {
    self.revision.with_lock(|revision| *revision)
  }

  /// Persists one durable state object and runs a one-shot callback after success.
  pub fn persist_state<F>(&self, state: S, on_persisted: F) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(&S, u64) -> Result<Behavior<M>, ActorError> + Send + 'static, {
    let mut store_ref = self.store_ref.clone();
    let expected_revision = self.revision();
    store_ref
      .try_tell(StateSourcedStoreCommand::PersistState { state, expected_revision, reply_to: self.reply_to.clone() })
      .map_err(|error| ActorError::fatal(format!("persist state send failed: {error:?}")))?;
    Ok(self.wait_for_persisted_state(Box::new(on_persisted)))
  }

  /// Deletes the durable state object and runs a one-shot callback after success.
  pub fn delete_state<F>(&self, on_deleted: F) -> Result<Behavior<M>, ActorError>
  where
    F: FnOnce(u64) -> Result<Behavior<M>, ActorError> + Send + 'static, {
    let mut store_ref = self.store_ref.clone();
    let expected_revision = self.revision();
    store_ref
      .try_tell(StateSourcedStoreCommand::DeleteState { expected_revision, reply_to: self.reply_to.clone() })
      .map_err(|error| ActorError::fatal(format!("delete state send failed: {error:?}")))?;
    Ok(self.wait_for_deleted_state(Box::new(on_deleted)))
  }

  fn wait_for_persisted_state(&self, callback: PersistStateCallback<S, M>) -> Behavior<M> {
    let adapter = match self.config.message_adapter().cloned() {
      | Some(adapter) => adapter,
      | None => return fatal_behavior("state-sourced message adapter is not configured"),
    };
    let callback = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(callback));
    let revision_cell = self.revision.clone();
    Behaviors::with_stash(self.config.stash_capacity(), move |stash| {
      let adapter = adapter.clone();
      let callback = callback.clone();
      let revision_cell = revision_cell.clone();
      Behaviors::receive_message(move |ctx, message| {
        if let Some(signal) = adapter.unwrap_signal(message) {
          return match signal {
            | StateSourcedEffectorSignal::StatePersisted { state, revision, .. } => {
              revision_cell.with_lock(|stored_revision| {
                *stored_revision = *revision;
              });
              let next = callback.with_lock(|slot| {
                slot.take().map(|callback| callback(state, *revision)).unwrap_or_else(|| Ok(Behaviors::same()))
              })?;
              stash.unstash_all(ctx)?;
              Ok(next)
            },
            | StateSourcedEffectorSignal::PersistenceFailed { error, .. } => Err(ActorError::fatal(error.to_string())),
            | _ => Ok(Behaviors::unhandled()),
          };
        }
        stash.stash(ctx)?;
        Ok(Behaviors::same())
      })
    })
  }

  fn wait_for_deleted_state(&self, callback: DeleteStateCallback<M>) -> Behavior<M> {
    let adapter = match self.config.message_adapter().cloned() {
      | Some(adapter) => adapter,
      | None => return fatal_behavior("state-sourced message adapter is not configured"),
    };
    let callback = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(callback));
    let revision_cell = self.revision.clone();
    Behaviors::with_stash(self.config.stash_capacity(), move |stash| {
      let adapter = adapter.clone();
      let callback = callback.clone();
      let revision_cell = revision_cell.clone();
      Behaviors::receive_message(move |ctx, message| {
        if let Some(signal) = adapter.unwrap_signal(message) {
          return match signal {
            | StateSourcedEffectorSignal::StateDeleted { revision, .. } => {
              revision_cell.with_lock(|stored_revision| {
                *stored_revision = *revision;
              });
              let next = callback.with_lock(|slot| {
                slot.take().map(|callback| callback(*revision)).unwrap_or_else(|| Ok(Behaviors::same()))
              })?;
              stash.unstash_all(ctx)?;
              Ok(next)
            },
            | StateSourcedEffectorSignal::PersistenceFailed { error, .. } => Err(ActorError::fatal(error.to_string())),
            | _ => Ok(Behaviors::unhandled()),
          };
        }
        stash.stash(ctx)?;
        Ok(Behaviors::same())
      })
    })
  }
}

impl<S, M> Clone for StateSourcedEffector<S, M>
where
  S: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self {
      config:    self.config.clone(),
      store_ref: self.store_ref.clone(),
      reply_to:  self.reply_to.clone(),
      revision:  self.revision.clone(),
      _message:  PhantomData,
    }
  }
}

fn fatal_behavior<M>(message: &'static str) -> Behavior<M>
where
  M: Send + Sync + 'static, {
  Behaviors::receive_message(move |_ctx, _message| Err(ActorError::fatal(message)))
}
