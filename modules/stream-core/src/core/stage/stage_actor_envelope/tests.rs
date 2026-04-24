use alloc::boxed::Box;

use fraktor_actor_core_rs::core::kernel::actor::{
  Pid,
  actor_ref::{ActorRef, ActorRefSenderShared, NullSender},
  messaging::AnyMessage,
};

use crate::core::stage::StageActorEnvelope;

fn actor_ref_with_pid(pid: Pid) -> ActorRef {
  ActorRef::new(pid, ActorRefSenderShared::new(Box::new(NullSender)))
}

#[test]
fn new_preserves_sender_and_message_payload() {
  // Given: stage actor へ届いた sender と user message
  let sender = ActorRef::null();
  let envelope = StageActorEnvelope::new(sender.clone(), AnyMessage::new(42_u32));

  // Then: Pekko StageActorRef.Receive の (ActorRef, Any) と同じ情報を保持する
  assert_eq!(envelope.sender().pid(), sender.pid());
  assert_eq!(envelope.message().downcast_ref::<u32>(), Some(&42_u32));
}

#[test]
fn sender_is_not_inferred_from_payload_sender() {
  // Given: AnyMessage 側にも sender が入っている message
  let payload_sender = ActorRef::null();
  let explicit_sender = actor_ref_with_pid(Pid::new(99, 0));
  let message = AnyMessage::new("payload").with_sender(payload_sender);

  // When: StageActorEnvelope を明示 sender で作る
  let envelope = StageActorEnvelope::new(explicit_sender.clone(), message);

  // Then: stage actor の receive tuple は envelope の sender を正として扱う
  assert_eq!(envelope.sender().pid(), explicit_sender.pid());
  assert_eq!(envelope.message().downcast_ref::<&'static str>(), Some(&"payload"));
}
