//! Internal typed state-sourced store actor.

#[cfg(test)]
#[path = "state_sourced_store_actor_test.rs"]
mod tests;

use alloc::{
  format,
  string::{String, ToString},
};
use core::{future::Future, marker::PhantomData};

use fraktor_actor_core_kernel_rs::actor::{error::ActorError, messaging::AnyMessage};
use fraktor_actor_core_typed_rs::{
  TypedActorRef, TypedProps,
  actor::{TypedActor, TypedActorContext},
};
use fraktor_persistence_core_kernel_rs::state::{DurableStateStoreProvider, GetObjectResult};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use super::state_sourced_store_command::{StateSourcedStore, StateSourcedStoreResult};
use crate::{
  StateSourcedEffectorConfig,
  internal::{StateSourcedStoreCommand, StateSourcedStoreReply},
};

type ReplyRef<S> = TypedActorRef<StateSourcedStoreReply<S>>;
type StoreProvider<S> = ArcShared<dyn DurableStateStoreProvider<S>>;
type StoreSlot<S> = SharedLock<Option<StateSourcedStore<S>>>;

pub(crate) struct StateSourcedStoreActor<S, M>
where
  S: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static, {
  config:   StateSourcedEffectorConfig<S, M>,
  store:    StoreSlot<S>,
  _message: PhantomData<fn() -> M>,
}

impl<S, M> StateSourcedStoreActor<S, M>
where
  S: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  pub(crate) fn props(
    config: StateSourcedEffectorConfig<S, M>,
    provider: StoreProvider<S>,
  ) -> TypedProps<StateSourcedStoreCommand<S>> {
    TypedProps::new(move || Self::new(config.clone(), provider.durable_state_store())).with_stash_mailbox()
  }

  fn new(config: StateSourcedEffectorConfig<S, M>, store: StateSourcedStore<S>) -> Self {
    Self { config, store: SharedLock::new_with_driver::<DefaultMutex<_>>(Some(store)), _message: PhantomData }
  }

  fn persistence_id(&self) -> String {
    self.config.persistence_id().as_str().to_string()
  }

  fn take_store(&mut self) -> Result<StateSourcedStore<S>, ActorError> {
    self
      .store
      .with_lock(Option::take)
      .ok_or_else(|| ActorError::recoverable("state-sourced store operation is already in flight"))
  }

  fn store_available(&self) -> bool {
    self.store.with_lock(|store| store.is_some())
  }

  fn recover(
    &mut self,
    ctx: &mut TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    let store = self.take_store()?;
    let store_slot = self.store.clone();
    let persistence_id = self.persistence_id();
    let future = async move {
      let result = store.get_object(persistence_id.as_str()).await;
      Self::restore_store(&store_slot, store);
      StateSourcedStoreCommand::RecoveryFinished { result, reply_to }
    };
    Self::pipe_to_self(ctx, future)
  }

  fn persist_state(
    &mut self,
    ctx: &mut TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    state: S,
    expected_revision: u64,
    tag: Option<String>,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    let store = self.take_store()?;
    let store_slot = self.store.clone();
    let persistence_id = self.persistence_id();
    let persisted_state = state.clone();
    let future = async move {
      let tag_ref = tag.as_deref();
      let mut store = store;
      let result = store.upsert_object(persistence_id.as_str(), expected_revision, persisted_state, tag_ref).await;
      Self::restore_store(&store_slot, store);
      StateSourcedStoreCommand::PersistStateFinished { state, expected_revision, result, reply_to }
    };
    Self::pipe_to_self(ctx, future)
  }

  fn delete_state(
    &mut self,
    ctx: &mut TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    expected_revision: u64,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    let store = self.take_store()?;
    let store_slot = self.store.clone();
    let persistence_id = self.persistence_id();
    let future = async move {
      let mut store = store;
      let result = store.delete_object(persistence_id.as_str(), expected_revision).await;
      Self::restore_store(&store_slot, store);
      StateSourcedStoreCommand::DeleteStateFinished { expected_revision, result, reply_to }
    };
    Self::pipe_to_self(ctx, future)
  }

  fn complete_recovery(
    &mut self,
    ctx: &TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    result: StateSourcedStoreResult<GetObjectResult<S>>,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    match result {
      | Ok(result) => {
        let revision = result.revision();
        Self::reply(&reply_to, StateSourcedStoreReply::RecoveryCompleted { state: result.into_value(), revision });
      },
      | Err(error) => Self::reply(&reply_to, StateSourcedStoreReply::RecoveryFailed { error }),
    }
    Self::unstash_all(ctx)
  }

  fn complete_persist(
    &mut self,
    ctx: &TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    state: S,
    expected_revision: u64,
    result: StateSourcedStoreResult<()>,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    match result {
      | Ok(()) => Self::reply(&reply_to, StateSourcedStoreReply::StatePersisted {
        state,
        revision: expected_revision.saturating_add(1),
      }),
      | Err(error) => Self::reply(&reply_to, StateSourcedStoreReply::PersistenceFailed { error }),
    }
    Self::unstash_all(ctx)
  }

  fn complete_delete(
    &mut self,
    ctx: &TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    _expected_revision: u64,
    result: StateSourcedStoreResult<()>,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    match result {
      | Ok(()) => Self::reply(&reply_to, StateSourcedStoreReply::StateDeleted { revision: 0 }),
      | Err(error) => Self::reply(&reply_to, StateSourcedStoreReply::PersistenceFailed { error }),
    }
    Self::unstash_all(ctx)
  }

  fn pipe_to_self(
    ctx: &mut TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    future: impl Future<Output = StateSourcedStoreCommand<S>> + Send + 'static,
  ) -> Result<(), ActorError> {
    ctx
      .as_untyped_mut()
      .pipe_to_self(future, AnyMessage::new)
      .map_err(|error| ActorError::recoverable(format!("state-sourced store pipe failed: {error}")))
  }

  fn reply(reply_to: &ReplyRef<S>, reply: StateSourcedStoreReply<S>) {
    let mut reply_to = reply_to.clone();
    reply_to.tell(reply);
  }

  fn unstash_all(ctx: &TypedActorContext<'_, StateSourcedStoreCommand<S>>) -> Result<(), ActorError> {
    ctx.unstash_all().map(|_count| ())
  }

  fn restore_store(store_slot: &StoreSlot<S>, store: StateSourcedStore<S>) {
    store_slot.with_lock(|slot| {
      *slot = Some(store);
    });
  }
}

impl<S, M> TypedActor<StateSourcedStoreCommand<S>> for StateSourcedStoreActor<S, M>
where
  S: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static,
{
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    message: &StateSourcedStoreCommand<S>,
  ) -> Result<(), ActorError> {
    if !self.store_available() && !message.is_completion() {
      return ctx.stash_with_limit(self.config.stash_capacity());
    }
    match message {
      | StateSourcedStoreCommand::Recover { reply_to } => self.recover(ctx, reply_to.clone()),
      | StateSourcedStoreCommand::RecoveryFinished { result, reply_to } => {
        self.complete_recovery(ctx, result.clone(), reply_to.clone())
      },
      | StateSourcedStoreCommand::PersistState { state, expected_revision, tag, reply_to } => {
        self.persist_state(ctx, state.clone(), *expected_revision, tag.clone(), reply_to.clone())
      },
      | StateSourcedStoreCommand::PersistStateFinished { state, expected_revision, result, reply_to } => {
        self.complete_persist(ctx, state.clone(), *expected_revision, result.clone(), reply_to.clone())
      },
      | StateSourcedStoreCommand::DeleteState { expected_revision, reply_to } => {
        self.delete_state(ctx, *expected_revision, reply_to.clone())
      },
      | StateSourcedStoreCommand::DeleteStateFinished { expected_revision, result, reply_to } => {
        self.complete_delete(ctx, *expected_revision, result.clone(), reply_to.clone())
      },
    }
  }
}
