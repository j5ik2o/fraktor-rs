//! Adapter bridging typed actor messages to kernel distributed-data protocol commands.

#[cfg(test)]
#[path = "replicator_message_adapter_test.rs"]
mod tests;

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
};
use core::time::Duration;

use fraktor_actor_core_typed_rs::{
  TypedActorRef,
  actor::{AskOnContextError, TypedActorContext},
  dsl::TypedAskError,
  message_adapter::AdapterError,
};
use fraktor_cluster_core_kernel_rs::ddata::{
  Delete, DeleteResponse, Get, GetReplicaCount, GetResponse, Key, ReadConsistency, ReplicaCount, ReplicatedData,
  Subscribe, SubscribeResponse, Unsubscribe, Update, UpdateResponse, WriteConsistency,
};

use crate::{ReplicatorCommand, update_modify_fn::UpdateModifyFn};

/// Adapts replicator responses to a typed actor's message protocol.
///
/// One adapter instance must be used from a single actor thread and for one
/// [`ReplicatedData`] type, matching Pekko's `ReplicatorMessageAdapter` contract.
pub struct ReplicatorMessageAdapter<'a, A, B, C = ()>
where
  A: Send + Sync + 'static,
  B: ReplicatedData + Send + Sync + 'static,
  C: Clone + Send + Sync + 'static, {
  context:                  &'a mut TypedActorContext<'a, A>,
  replicator:               TypedActorRef<ReplicatorCommand<B, C>>,
  unexpected_ask_timeout:   Duration,
  changed_message_adapters: BTreeMap<String, TypedActorRef<SubscribeResponse<B>>>,
}

impl<'a, A, B, C> ReplicatorMessageAdapter<'a, A, B, C>
where
  A: Send + Sync + 'static,
  B: ReplicatedData + Send + Sync + 'static,
  C: Clone + Send + Sync + 'static,
{
  /// Creates a new adapter bound to the requesting actor context.
  #[must_use]
  pub fn new(
    context: &'a mut TypedActorContext<'a, A>,
    replicator: TypedActorRef<ReplicatorCommand<B, C>>,
    unexpected_ask_timeout: Duration,
  ) -> Self {
    Self { context, replicator, unexpected_ask_timeout, changed_message_adapters: BTreeMap::new() }
  }

  /// Subscribes to changes for the given key.
  ///
  /// # Errors
  ///
  /// Returns an error when the subscribe adapter cannot be registered or sent.
  pub fn subscribe<F>(&mut self, key: Key<B>, response_adapter: F) -> Result<(), AdapterError>
  where
    F: Fn(SubscribeResponse<B>) -> Result<A, AdapterError> + Send + Sync + 'static, {
    self.unsubscribe(key.clone())?;
    let subscriber = self.context.message_adapter(response_adapter)?;
    self.replicator.tell(ReplicatorCommand::subscribe(Subscribe::new(key.clone(), subscriber.clone())));
    self.changed_message_adapters.insert(key.id().to_string(), subscriber);
    Ok(())
  }

  /// Unsubscribes from a previous subscription for the given key.
  ///
  /// # Errors
  ///
  /// Returns an error when the unsubscribe command cannot be sent.
  pub fn unsubscribe(&mut self, key: Key<B>) -> Result<(), AdapterError> {
    if let Some(subscriber) = self.changed_message_adapters.remove(key.id()) {
      self.replicator.tell(ReplicatorCommand::unsubscribe(Unsubscribe::new(key, subscriber)));
    }
    Ok(())
  }

  /// Sends an update request and adapts the response back to the actor protocol.
  ///
  /// # Errors
  ///
  /// Returns an error when the ask operation cannot be started.
  pub fn ask_update<FCreate, FAdapt>(
    &mut self,
    create_request: FCreate,
    response_adapter: FAdapt,
  ) -> Result<(), AskOnContextError>
  where
    FCreate: FnOnce(TypedActorRef<UpdateResponse<B, C>>) -> (Update<B, C>, UpdateModifyFn<B>),
    FAdapt: Fn(Result<UpdateResponse<B, C>, TypedAskError>) -> A + Send + Sync + 'static, {
    let mut replicator = self.replicator.clone();
    self.context.ask(
      &mut replicator,
      |reply_to| {
        let (command, modify) = create_request(reply_to.clone());
        ReplicatorCommand::update(command, modify, reply_to)
      },
      response_adapter,
      self.unexpected_ask_timeout,
    )
  }

  /// Sends a get request and adapts the response back to the actor protocol.
  ///
  /// # Errors
  ///
  /// Returns an error when the ask operation cannot be started.
  pub fn ask_get<FCreate, FAdapt>(
    &mut self,
    create_request: FCreate,
    response_adapter: FAdapt,
  ) -> Result<(), AskOnContextError>
  where
    FCreate: FnOnce(TypedActorRef<GetResponse<B, C>>) -> Get<B, C>,
    FAdapt: Fn(Result<GetResponse<B, C>, TypedAskError>) -> A + Send + Sync + 'static, {
    let mut replicator = self.replicator.clone();
    self.context.ask(
      &mut replicator,
      |reply_to| ReplicatorCommand::get(create_request(reply_to.clone()), reply_to),
      response_adapter,
      self.unexpected_ask_timeout,
    )
  }

  /// Sends a delete request and adapts the response back to the actor protocol.
  ///
  /// # Errors
  ///
  /// Returns an error when the ask operation cannot be started.
  pub fn ask_delete<FCreate, FAdapt>(
    &mut self,
    create_request: FCreate,
    response_adapter: FAdapt,
  ) -> Result<(), AskOnContextError>
  where
    FCreate: FnOnce(TypedActorRef<DeleteResponse<B, C>>) -> Delete<B, C>,
    FAdapt: Fn(Result<DeleteResponse<B, C>, TypedAskError>) -> A + Send + Sync + 'static, {
    let mut replicator = self.replicator.clone();
    self.context.ask(
      &mut replicator,
      |reply_to| ReplicatorCommand::delete(create_request(reply_to.clone()), reply_to),
      response_adapter,
      self.unexpected_ask_timeout,
    )
  }

  /// Sends a replica-count query and adapts the response back to the actor protocol.
  ///
  /// # Errors
  ///
  /// Returns an error when the ask operation cannot be started.
  pub fn ask_replica_count<FCreate, FAdapt>(
    &mut self,
    create_request: FCreate,
    response_adapter: FAdapt,
  ) -> Result<(), AskOnContextError>
  where
    FCreate: FnOnce(TypedActorRef<ReplicaCount>) -> GetReplicaCount,
    FAdapt: Fn(Result<ReplicaCount, TypedAskError>) -> A + Send + Sync + 'static, {
    let mut replicator = self.replicator.clone();
    self.context.ask(
      &mut replicator,
      |reply_to| ReplicatorCommand::get_replica_count(create_request(reply_to.clone()), reply_to),
      response_adapter,
      self.unexpected_ask_timeout,
    )
  }

  /// Convenience helper for update requests built from key, consistency, and modify function.
  ///
  /// # Errors
  ///
  /// Returns an error when the ask operation cannot be started.
  pub fn ask_update_with<FModify, FAdapt>(
    &mut self,
    key: Key<B>,
    consistency: WriteConsistency,
    modify: FModify,
    response_adapter: FAdapt,
  ) -> Result<(), AskOnContextError>
  where
    FModify: Fn(Option<&B>) -> Result<B, String> + Send + Sync + 'static,
    FAdapt: Fn(Result<UpdateResponse<B, C>, TypedAskError>) -> A + Send + Sync + 'static, {
    let modify = UpdateModifyFn::new(modify);
    self.ask_update(move |_reply_to| (Update::new(key, consistency), modify), response_adapter)
  }

  /// Convenience helper for get requests built from key and consistency.
  ///
  /// # Errors
  ///
  /// Returns an error when the ask operation cannot be started.
  pub fn ask_get_with<FAdapt>(
    &mut self,
    key: Key<B>,
    consistency: ReadConsistency,
    response_adapter: FAdapt,
  ) -> Result<(), AskOnContextError>
  where
    FAdapt: Fn(Result<GetResponse<B, C>, TypedAskError>) -> A + Send + Sync + 'static, {
    self.ask_get(move |_reply_to| Get::new(key, consistency), response_adapter)
  }

  /// Convenience helper for delete requests built from key and consistency.
  ///
  /// # Errors
  ///
  /// Returns an error when the ask operation cannot be started.
  pub fn ask_delete_with<FAdapt>(
    &mut self,
    key: Key<B>,
    consistency: WriteConsistency,
    response_adapter: FAdapt,
  ) -> Result<(), AskOnContextError>
  where
    FAdapt: Fn(Result<DeleteResponse<B, C>, TypedAskError>) -> A + Send + Sync + 'static, {
    self.ask_delete(move |_reply_to| Delete::new(key, consistency), response_adapter)
  }
}
