use fraktor_actor_core_rs::actor::actor_ref::ActorRef;

use crate::{
  TypedActorRef,
  delivery::{DurableProducerQueue, DurableProducerQueueCommand, DurableProducerQueueState, MessageSent, NO_QUALIFIER},
};

fn make_typed_ref<M>() -> TypedActorRef<M>
where
  M: Send + Sync + 'static, {
  TypedActorRef::from_untyped(ActorRef::null())
}

#[test]
fn durable_producer_queue_facade_creates_load_state_command() {
  let reply_to = make_typed_ref::<DurableProducerQueueState<u32>>();
  let command = DurableProducerQueue::load_state(reply_to);

  assert!(matches!(command, DurableProducerQueueCommand::LoadState { .. }));
}

#[test]
fn durable_producer_queue_facade_exposes_empty_state() {
  let state = DurableProducerQueue::empty_state::<u32>();

  assert_eq!(state.current_seq_nr(), 1);
  assert!(state.unconfirmed().is_empty());
}

#[test]
fn durable_producer_queue_facade_creates_store_commands() {
  let reply_to = make_typed_ref();
  let sent = MessageSent::new(1, 10_u32, true, NO_QUALIFIER.clone(), 99);

  let store_sent = DurableProducerQueue::store_message_sent(sent, reply_to);
  let store_confirmed = DurableProducerQueue::store_message_confirmed::<u32>(1, NO_QUALIFIER.clone(), 101);

  assert!(matches!(store_sent, DurableProducerQueueCommand::StoreMessageSent { .. }));
  assert!(matches!(store_confirmed, DurableProducerQueueCommand::StoreMessageConfirmed { .. }));
}
