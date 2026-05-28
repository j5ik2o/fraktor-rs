//! Internal event-sourced store commands.

use alloc::vec::Vec;

use fraktor_actor_core_typed_rs::TypedActorRef;

use crate::internal::EventSourcedStoreReply;

pub(crate) enum EventSourcedStoreCommand<S, E>
where
  S: Send + Sync + 'static,
  E: Send + Sync + 'static, {
  PersistEvent { event: E, reply_to: TypedActorRef<EventSourcedStoreReply<S, E>> },
  PersistEvents { events: Vec<E>, reply_to: TypedActorRef<EventSourcedStoreReply<S, E>> },
  PersistSnapshot { snapshot: S, reply_to: TypedActorRef<EventSourcedStoreReply<S, E>> },
  DeleteSnapshots { to_sequence_nr: u64, reply_to: TypedActorRef<EventSourcedStoreReply<S, E>> },
  DeleteEvents { to_sequence_nr: u64, reply_to: TypedActorRef<EventSourcedStoreReply<S, E>> },
}
