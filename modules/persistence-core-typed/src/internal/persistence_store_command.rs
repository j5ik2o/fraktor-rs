//! Internal persistence store commands.

use alloc::vec::Vec;

use fraktor_actor_core_typed_rs::TypedActorRef;

use super::PersistenceStoreReply;

pub(crate) enum PersistenceStoreCommand<S, E>
where
  S: Send + Sync + 'static,
  E: Send + Sync + 'static, {
  PersistEvent { event: E, reply_to: TypedActorRef<PersistenceStoreReply<S, E>> },
  PersistEvents { events: Vec<E>, reply_to: TypedActorRef<PersistenceStoreReply<S, E>> },
  PersistSnapshot { snapshot: S, reply_to: TypedActorRef<PersistenceStoreReply<S, E>> },
  DeleteSnapshots { to_sequence_nr: u64, reply_to: TypedActorRef<PersistenceStoreReply<S, E>> },
}
