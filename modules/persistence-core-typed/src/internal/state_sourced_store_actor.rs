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

use crate::{
  StateSourcedEffectorConfig,
  internal::{
    StateSourcedStoreCommand, StateSourcedStoreReply,
    state_sourced_store_command::{StateSourcedStore, StateSourcedStoreResult},
  },
};

type ReplyRef<S> = TypedActorRef<StateSourcedStoreReply<S>>;
type StoreProvider<S> = ArcShared<dyn DurableStateStoreProvider<S>>;
type StoreSlot<S> = SharedLock<Option<StateSourcedStore<S>>>;

fn restore_store<S>(store_slot: &StoreSlot<S>, store: StateSourcedStore<S>)
where
  S: Send + Sync + 'static, {
  store_slot.with_lock(|slot| {
    *slot = Some(store);
  });
}

struct StoreLease<S>
where
  S: Send + Sync + 'static, {
  store:      Option<StateSourcedStore<S>>,
  store_slot: StoreSlot<S>,
}

impl<S> StoreLease<S>
where
  S: Send + Sync + 'static,
{
  fn new(store: StateSourcedStore<S>, store_slot: StoreSlot<S>) -> Self {
    Self { store: Some(store), store_slot }
  }

  fn store_mut(&mut self) -> &mut StateSourcedStore<S> {
    self.store.as_mut().expect("leased store should be present")
  }

  fn restore(mut self) {
    let store = self.store.take().expect("leased store should be present");
    restore_store(&self.store_slot, store);
  }
}

impl<S> Drop for StoreLease<S>
where
  S: Send + Sync + 'static,
{
  fn drop(&mut self) {
    if let Some(store) = self.store.take() {
      restore_store(&self.store_slot, store);
    }
  }
}

pub(crate) struct StateSourcedStoreActor<S, M>
where
  S: Clone + Send + Sync + 'static,
  M: Send + Sync + 'static, {
  config:    StateSourcedEffectorConfig<S, M>,
  store:     StoreSlot<S>,
  in_flight: bool,
  _message:  PhantomData<fn() -> M>,
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
    Self {
      config,
      store: SharedLock::new_with_driver::<DefaultMutex<_>>(Some(store)),
      in_flight: false,
      _message: PhantomData,
    }
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

  fn recover(
    &mut self,
    ctx: &mut TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    let store = self.take_store()?;
    let mut store_lease = StoreLease::new(store, self.store.clone());
    let persistence_id = self.persistence_id();
    let future = async move {
      let result = store_lease.store_mut().get_object(persistence_id.as_str()).await;
      store_lease.restore();
      StateSourcedStoreCommand::RecoveryFinished { result, reply_to }
    };
    Self::pipe_to_self(ctx, future)?;
    self.in_flight = true;
    Ok(())
  }

  fn persist_state(
    &mut self,
    ctx: &mut TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    state: S,
    expected_revision: u64,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    let store = self.take_store()?;
    let mut store_lease = StoreLease::new(store, self.store.clone());
    let persistence_id = self.persistence_id();
    let persisted_state = state.clone();
    let future = async move {
      let result =
        store_lease.store_mut().upsert_object(persistence_id.as_str(), expected_revision, persisted_state, None).await;
      store_lease.restore();
      StateSourcedStoreCommand::PersistStateFinished { state, expected_revision, result, reply_to }
    };
    Self::pipe_to_self(ctx, future)?;
    self.in_flight = true;
    Ok(())
  }

  fn delete_state(
    &mut self,
    ctx: &mut TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    expected_revision: u64,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    let store = self.take_store()?;
    let mut store_lease = StoreLease::new(store, self.store.clone());
    let persistence_id = self.persistence_id();
    let future = async move {
      let result = store_lease.store_mut().delete_object(persistence_id.as_str(), expected_revision).await;
      store_lease.restore();
      StateSourcedStoreCommand::DeleteStateFinished { result, reply_to }
    };
    Self::pipe_to_self(ctx, future)?;
    self.in_flight = true;
    Ok(())
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
    self.in_flight = false;
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
    self.in_flight = false;
    Self::unstash_all(ctx)
  }

  fn complete_delete(
    &mut self,
    ctx: &TypedActorContext<'_, StateSourcedStoreCommand<S>>,
    result: StateSourcedStoreResult<()>,
    reply_to: ReplyRef<S>,
  ) -> Result<(), ActorError> {
    match result {
      | Ok(()) => {
        // DurableStateStore::delete_object resets the object revision to 0.
        let deleted_revision = 0;
        Self::reply(&reply_to, StateSourcedStoreReply::StateDeleted { revision: deleted_revision });
      },
      | Err(error) => Self::reply(&reply_to, StateSourcedStoreReply::PersistenceFailed { error }),
    }
    self.in_flight = false;
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
    if self.in_flight && !message.is_completion() {
      return ctx.stash_with_limit(self.config.stash_capacity());
    }
    match message {
      | StateSourcedStoreCommand::Recover { reply_to } => self.recover(ctx, reply_to.clone()),
      | StateSourcedStoreCommand::RecoveryFinished { result, reply_to } => {
        self.complete_recovery(ctx, result.clone(), reply_to.clone())
      },
      | StateSourcedStoreCommand::PersistState { state, expected_revision, reply_to } => {
        self.persist_state(ctx, state.clone(), *expected_revision, reply_to.clone())
      },
      | StateSourcedStoreCommand::PersistStateFinished { state, expected_revision, result, reply_to } => {
        self.complete_persist(ctx, state.clone(), *expected_revision, result.clone(), reply_to.clone())
      },
      | StateSourcedStoreCommand::DeleteState { expected_revision, reply_to } => {
        self.delete_state(ctx, *expected_revision, reply_to.clone())
      },
      | StateSourcedStoreCommand::DeleteStateFinished { result, reply_to } => {
        self.complete_delete(ctx, result.clone(), reply_to.clone())
      },
    }
  }
}
