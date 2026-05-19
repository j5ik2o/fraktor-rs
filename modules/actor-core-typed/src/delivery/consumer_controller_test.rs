use alloc::{string::String, vec::Vec};

use fraktor_actor_core_kernel_rs::actor::{Pid, actor_ref::NullSender};

use super::{ConsumerController, ConsumerControllerState, collect_on_sequenced_message};
use crate::{
  TypedActorRef,
  delivery::{
    ConsumerControllerCommand, ConsumerControllerConfig, ConsumerControllerConfirmed, ProducerControllerCommand, SeqNr,
    SequencedMessage,
  },
};

fn make_typed_ref<M: Send + Sync + 'static>() -> TypedActorRef<M> {
  TypedActorRef::from_untyped(crate::test_support::actor_ref_with_sender(Pid::new(1, 0), NullSender))
}

fn make_sequenced_message(seq_nr: SeqNr) -> SequencedMessage<u32> {
  SequencedMessage::new(
    String::from("test-producer"),
    seq_nr,
    seq_nr as u32,
    false,
    false,
    make_typed_ref::<ProducerControllerCommand<u32>>(),
  )
}

#[test]
fn consumer_controller_factory_methods_compile() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<ConsumerControllerCommand<String>>();

  // 具体的な型でビヘイビアファクトリがコンパイルできることを確認する。
  let _behavior = ConsumerController::behavior::<u32>();
}

#[test]
fn requested_window_accepts_messages_even_when_confirmation_lags() {
  let settings = ConsumerControllerConfig::new().with_flow_control_window(3);
  let mut state = ConsumerControllerState::<u32>::new(settings);
  state.confirmed_seq_nr = 10;
  state.received_seq_nr = 12;
  state.requested_seq_nr = 15;
  state.producer_controller = Some(make_typed_ref::<ProducerControllerCommand<u32>>());

  let self_ref = make_typed_ref::<ConsumerControllerCommand<u32>>();
  let confirm_adapter = make_typed_ref::<ConsumerControllerConfirmed>();
  let mut deferred = Vec::new();

  collect_on_sequenced_message(&mut state, make_sequenced_message(15), &self_ref, &confirm_adapter, &mut deferred);

  assert_eq!(state.stashed.len(), 1);
  assert_eq!(state.stashed[0].seq_nr(), 15);
}

#[test]
fn requested_window_rejects_messages_above_requested_sequence() {
  let settings = ConsumerControllerConfig::new().with_flow_control_window(3);
  let mut state = ConsumerControllerState::<u32>::new(settings);
  state.confirmed_seq_nr = 10;
  state.received_seq_nr = 12;
  state.requested_seq_nr = 15;
  state.producer_controller = Some(make_typed_ref::<ProducerControllerCommand<u32>>());

  let self_ref = make_typed_ref::<ConsumerControllerCommand<u32>>();
  let confirm_adapter = make_typed_ref::<ConsumerControllerConfirmed>();
  let mut deferred = Vec::new();

  collect_on_sequenced_message(&mut state, make_sequenced_message(16), &self_ref, &confirm_adapter, &mut deferred);

  assert!(state.stashed.is_empty());
  assert_eq!(state.received_seq_nr, 12);
}
