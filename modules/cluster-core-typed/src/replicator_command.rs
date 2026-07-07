//! Replicator command envelope for typed distributed-data interaction.

use fraktor_actor_core_typed_rs::TypedActorRef;
use fraktor_cluster_core_kernel_rs::ddata::{
  Delete, DeleteResponse, FlushChanges, Get, GetReplicaCount, GetResponse, Key, ReadConsistency, ReplicaCount,
  ReplicatedData, Subscribe, SubscribeResponse, Unsubscribe, Update, UpdateResponse, WriteConsistency,
};

use crate::update_modify_fn::UpdateModifyFn;

/// Command protocol accepted by the distributed-data replicator actor.
pub enum ReplicatorCommand<D: ReplicatedData + Send + Sync + 'static, C = ()>
where
  C: Clone + Send + Sync + 'static, {
  /// Reads one CRDT value.
  Get {
    /// Get metadata.
    command:  Get<D, C>,
    /// Reply destination.
    reply_to: TypedActorRef<GetResponse<D, C>>,
  },
  /// Updates one CRDT value.
  Update {
    /// Update metadata.
    command:  Update<D, C>,
    /// Modify function applied by the replicator.
    modify:   UpdateModifyFn<D>,
    /// Reply destination.
    reply_to: TypedActorRef<UpdateResponse<D, C>>,
  },
  /// Deletes one CRDT value.
  Delete {
    /// Delete metadata.
    command:  Delete<D, C>,
    /// Reply destination.
    reply_to: TypedActorRef<DeleteResponse<D, C>>,
  },
  /// Registers a change subscriber.
  Subscribe(Subscribe<D, TypedActorRef<SubscribeResponse<D>>>),
  /// Unregisters a change subscriber.
  Unsubscribe(Unsubscribe<D, TypedActorRef<SubscribeResponse<D>>>),
  /// Queries the current replica count.
  GetReplicaCount {
    /// Reply destination.
    reply_to: TypedActorRef<ReplicaCount>,
  },
  /// Requests immediate subscriber notification flush.
  FlushChanges(FlushChanges),
}

impl<D: ReplicatedData + Send + Sync + 'static, C: Clone + Send + Sync + 'static> Clone for ReplicatorCommand<D, C> {
  fn clone(&self) -> Self {
    match self {
      | Self::Get { command, reply_to } => Self::Get { command: command.clone(), reply_to: reply_to.clone() },
      | Self::Update { command, modify, reply_to } => {
        Self::Update { command: command.clone(), modify: modify.clone(), reply_to: reply_to.clone() }
      },
      | Self::Delete { command, reply_to } => Self::Delete { command: command.clone(), reply_to: reply_to.clone() },
      | Self::Subscribe(command) => Self::Subscribe(command.clone()),
      | Self::Unsubscribe(command) => Self::Unsubscribe(command.clone()),
      | Self::GetReplicaCount { reply_to } => Self::GetReplicaCount { reply_to: reply_to.clone() },
      | Self::FlushChanges(value) => Self::FlushChanges(*value),
    }
  }
}

impl<D: ReplicatedData + Send + Sync + 'static, C: Clone + Send + Sync + 'static> core::fmt::Debug
  for ReplicatorCommand<D, C>
{
  fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | Self::Get { command, .. } => formatter.debug_struct("Get").field("key", &command.key().id()).finish(),
      | Self::Update { command, .. } => formatter.debug_struct("Update").field("key", &command.key().id()).finish(),
      | Self::Delete { command, .. } => formatter.debug_struct("Delete").field("key", &command.key().id()).finish(),
      | Self::Subscribe(command) => formatter.debug_struct("Subscribe").field("key", &command.key().id()).finish(),
      | Self::Unsubscribe(command) => formatter.debug_struct("Unsubscribe").field("key", &command.key().id()).finish(),
      | Self::GetReplicaCount { .. } => formatter.write_str("GetReplicaCount"),
      | Self::FlushChanges(value) => formatter.debug_tuple("FlushChanges").field(value).finish(),
    }
  }
}

impl<D: ReplicatedData + Send + Sync + 'static, C: Clone + Send + Sync + 'static> ReplicatorCommand<D, C> {
  /// Creates a get command with reply destination.
  #[must_use]
  pub fn get(command: Get<D, C>, reply_to: TypedActorRef<GetResponse<D, C>>) -> Self {
    Self::Get { command, reply_to }
  }

  /// Creates an update command with modify function and reply destination.
  #[must_use]
  pub fn update(
    command: Update<D, C>,
    modify: UpdateModifyFn<D>,
    reply_to: TypedActorRef<UpdateResponse<D, C>>,
  ) -> Self {
    Self::Update { command, modify, reply_to }
  }

  /// Creates a delete command with reply destination.
  #[must_use]
  pub fn delete(command: Delete<D, C>, reply_to: TypedActorRef<DeleteResponse<D, C>>) -> Self {
    Self::Delete { command, reply_to }
  }

  /// Creates a subscribe command.
  #[must_use]
  pub fn subscribe(command: Subscribe<D, TypedActorRef<SubscribeResponse<D>>>) -> Self {
    Self::Subscribe(command)
  }

  /// Creates an unsubscribe command.
  #[must_use]
  pub fn unsubscribe(command: Unsubscribe<D, TypedActorRef<SubscribeResponse<D>>>) -> Self {
    Self::Unsubscribe(command)
  }

  /// Creates a replica-count query command.
  #[must_use]
  pub fn get_replica_count(_request: GetReplicaCount, reply_to: TypedActorRef<ReplicaCount>) -> Self {
    let _ = _request;
    Self::GetReplicaCount { reply_to }
  }

  /// Creates a read command using the provided key and consistency.
  #[must_use]
  pub fn read(key: Key<D>, consistency: ReadConsistency, reply_to: TypedActorRef<GetResponse<D, C>>) -> Self {
    Self::get(Get::new(key, consistency), reply_to)
  }

  /// Creates a write command using the provided key and consistency.
  #[must_use]
  pub fn write(
    key: Key<D>,
    consistency: WriteConsistency,
    modify: UpdateModifyFn<D>,
    reply_to: TypedActorRef<UpdateResponse<D, C>>,
  ) -> Self {
    Self::update(Update::new(key, consistency), modify, reply_to)
  }

  /// Creates a delete command using the provided key and consistency.
  #[must_use]
  pub fn remove(key: Key<D>, consistency: WriteConsistency, reply_to: TypedActorRef<DeleteResponse<D, C>>) -> Self {
    Self::delete(Delete::new(key, consistency), reply_to)
  }
}
