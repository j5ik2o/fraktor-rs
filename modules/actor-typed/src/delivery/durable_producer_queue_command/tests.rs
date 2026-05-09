use alloc::string::String;

use fraktor_actor_core_rs::actor::{Pid, actor_ref::NullSender};

use crate::{
  TypedActorRef,
  delivery::{DurableProducerQueueCommand, DurableProducerQueueState, MessageSent, NO_QUALIFIER, StoreMessageSentAck},
};

fn make_typed_ref<M: Send + Sync + 'static>() -> TypedActorRef<M> {
  TypedActorRef::from_untyped(crate::test_support::actor_ref_with_sender(Pid::new(1, 0), NullSender))
}

// ---------------------------------------------------------------------------
// LoadState variant
// ---------------------------------------------------------------------------

#[test]
fn load_state_variant_is_constructible() {
  // Given: a reply_to ref for State
  let reply_to = make_typed_ref::<DurableProducerQueueState<u32>>();

  // When
  let cmd = DurableProducerQueueCommand::<u32>::load_state(reply_to);

  // Then
  assert!(matches!(cmd, DurableProducerQueueCommand::LoadState { .. }));
}

// ---------------------------------------------------------------------------
// StoreMessageSent variant
// ---------------------------------------------------------------------------

#[test]
fn store_message_sent_variant_is_constructible() {
  // Given
  let sent = MessageSent::new(1, 42_u32, false, NO_QUALIFIER.clone(), 100);
  let reply_to = make_typed_ref::<StoreMessageSentAck>();

  // When
  let cmd = DurableProducerQueueCommand::store_message_sent(sent, reply_to);

  // Then
  assert!(matches!(cmd, DurableProducerQueueCommand::StoreMessageSent { .. }));
}

// ---------------------------------------------------------------------------
// StoreMessageConfirmed variant
// ---------------------------------------------------------------------------

#[test]
fn store_message_confirmed_variant_is_constructible() {
  // Given/When
  let cmd = DurableProducerQueueCommand::<u32>::store_message_confirmed(5, String::from("topic-A"), 2000);

  // Then
  assert!(matches!(cmd, DurableProducerQueueCommand::StoreMessageConfirmed { .. }));
}

#[test]
fn store_message_confirmed_with_no_qualifier() {
  // Given/When
  let cmd = DurableProducerQueueCommand::<u32>::store_message_confirmed(1, NO_QUALIFIER.clone(), 0);

  // Then
  assert!(matches!(cmd, DurableProducerQueueCommand::StoreMessageConfirmed { .. }));
}

// ---------------------------------------------------------------------------
// Variant distinction
// ---------------------------------------------------------------------------

#[test]
fn command_variants_are_distinct() {
  // Given
  let reply_state = make_typed_ref::<DurableProducerQueueState<u32>>();
  let reply_ack = make_typed_ref::<StoreMessageSentAck>();
  let sent = MessageSent::new(1, 0_u32, false, NO_QUALIFIER.clone(), 0);

  let load = DurableProducerQueueCommand::<u32>::load_state(reply_state);
  let store_sent = DurableProducerQueueCommand::store_message_sent(sent, reply_ack);
  let store_confirmed = DurableProducerQueueCommand::<u32>::store_message_confirmed(1, NO_QUALIFIER.clone(), 0);

  // Then: each matches only its own variant
  assert!(matches!(load, DurableProducerQueueCommand::LoadState { .. }));
  assert!(!matches!(load, DurableProducerQueueCommand::StoreMessageSent { .. }));

  assert!(matches!(store_sent, DurableProducerQueueCommand::StoreMessageSent { .. }));
  assert!(!matches!(store_sent, DurableProducerQueueCommand::StoreMessageConfirmed { .. }));

  assert!(matches!(store_confirmed, DurableProducerQueueCommand::StoreMessageConfirmed { .. }));
  assert!(!matches!(store_confirmed, DurableProducerQueueCommand::LoadState { .. }));
}

// ---------------------------------------------------------------------------
// Debug
// ---------------------------------------------------------------------------

#[test]
fn command_debug_format_is_non_empty() {
  // Given
  let cmd = DurableProducerQueueCommand::<u32>::store_message_confirmed(1, NO_QUALIFIER.clone(), 0);

  // When
  let debug_str = alloc::format!("{:?}", cmd);

  // Then
  assert!(!debug_str.is_empty());
}
