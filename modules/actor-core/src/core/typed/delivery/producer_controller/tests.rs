use alloc::{string::String, vec::Vec};

use super::{
  DeferredAction, DurableQueueTimeout, ProducerControllerState, collect_on_durable_queue_message_stored, collect_on_msg,
};
use crate::core::{
  kernel::actor::{
    Pid,
    actor_ref::{ActorRef, NullSender},
  },
  typed::{
    TypedActorRef,
    delivery::{
      ConsumerControllerCommand, DurableProducerQueueCommand, ProducerController, ProducerControllerCommand,
      ProducerControllerRequestNext, StoreMessageSentAck,
    },
  },
};

fn make_typed_ref<M: Send + Sync + 'static>() -> TypedActorRef<M> {
  TypedActorRef::from_untyped(ActorRef::new_with_builtin_lock(Pid::new(1, 0), NullSender))
}

#[test]
fn producer_controller_factory_methods_compile() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ProducerControllerCommand<String>>();

  // 具体的な型でビヘイビアファクトリがコンパイルできることを確認する。
  let _behavior = ProducerController::behavior::<u32>("test-producer");
}

#[test]
fn behavior_with_durable_queue_none_compiles() {
  // durable_queue が None の場合は behavior() と同等の動作であること。
  let _behavior = ProducerController::behavior_with_durable_queue::<u32>("test-producer", None);
}

#[test]
fn durable_queue_ack_releases_pending_delivery() {
  let mut state = ProducerControllerState::<u32>::new("test-producer".to_string());
  state.consumer_controller = Some(make_typed_ref::<ConsumerControllerCommand<u32>>());
  state.producer = Some(make_typed_ref::<ProducerControllerRequestNext<u32>>());
  state.durable_queue = Some(make_typed_ref::<DurableProducerQueueCommand<u32>>());
  state.store_ack_adapter = Some(make_typed_ref::<StoreMessageSentAck>());

  let self_ref = make_typed_ref::<ProducerControllerCommand<u32>>();
  let mut deferred = Vec::new();

  collect_on_msg(&mut state, 42_u32, &self_ref, &mut deferred);

  assert!(state.pending_delivery.is_some());
  assert!(state.unconfirmed.is_empty());
  assert_eq!(state.current_seq_nr, 2);
  assert!(matches!(deferred.as_slice(), [DeferredAction::TellDurableQueue {
    timeout: Some(DurableQueueTimeout::Store { seq_nr: 1, attempt: 1 }),
    ..
  }]));

  let mut released = Vec::new();
  collect_on_durable_queue_message_stored(&mut state, &StoreMessageSentAck::new(1), &mut released);

  assert!(state.pending_delivery.is_none());
  assert_eq!(state.unconfirmed.len(), 1);
  assert!(!state.send_first);
  assert!(matches!(released.as_slice(), [DeferredAction::SendSequenced(_, _)]));
}
