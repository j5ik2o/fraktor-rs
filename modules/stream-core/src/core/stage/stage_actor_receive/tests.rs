use alloc::{boxed::Box, vec, vec::Vec};

use fraktor_actor_core_kernel_rs::actor::{actor_ref::ActorRef, messaging::AnyMessage};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::{
  StreamError,
  stage::{StageActorEnvelope, StageActorReceive},
};

struct RecordingReceive {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl StageActorReceive for RecordingReceive {
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    if let Some(value) = envelope.message().downcast_ref::<u32>() {
      self.values.lock().push(*value);
    }
    Ok(())
  }
}

struct FailingReceive;

impl StageActorReceive for FailingReceive {
  fn receive(&mut self, _envelope: StageActorEnvelope) -> Result<(), StreamError> {
    Err(StreamError::Failed)
  }
}

#[test]
fn receive_trait_is_object_safe_and_receives_envelopes() {
  // Given: Box<dyn StageActorReceive> として保持した receive callback
  let values = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let mut receive: Box<dyn StageActorReceive> = Box::new(RecordingReceive { values: values.clone() });
  let envelope = StageActorEnvelope::new(ActorRef::null(), AnyMessage::new(7_u32));

  // When: trait object 経由で message を渡す
  receive.receive(envelope).expect("receive succeeds");

  // Then: callback は typed payload を観測できる
  assert_eq!(*values.lock(), vec![7_u32]);
}

#[test]
fn receive_failure_is_returned_to_stage_actor_boundary() {
  // Given: 失敗を返す receive callback
  let mut receive: Box<dyn StageActorReceive> = Box::new(FailingReceive);
  let envelope = StageActorEnvelope::new(ActorRef::null(), AnyMessage::new(1_u32));

  // When: receive を実行する
  let result = receive.receive(envelope);

  // Then: StageActor は失敗を握りつぶさず境界へ返せる
  assert_eq!(result, Err(StreamError::Failed));
}
