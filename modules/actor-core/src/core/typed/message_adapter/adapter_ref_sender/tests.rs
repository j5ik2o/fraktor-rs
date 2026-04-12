use alloc::vec::Vec;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::{
  kernel::{
    actor::{
      Pid,
      actor_ref::{ActorRefSender, ActorRefSenderSharedFactory},
      error::SendError,
      messaging::AnyMessage,
    },
    system::{ActorSystem, shared_factory::BuiltinSpinSharedFactory},
  },
  typed::message_adapter::{AdapterEnvelope, AdapterLifecycleState, AdapterRefSender, adapter_ref_sender::SendOutcome},
};

struct ProbeSender {
  messages: ArcShared<SpinSyncMutex<Vec<AnyMessage>>>,
}

impl ProbeSender {
  fn new(messages: ArcShared<SpinSyncMutex<Vec<AnyMessage>>>) -> Self {
    Self { messages }
  }
}

impl ActorRefSender for ProbeSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

#[test]
fn adapter_sender_wraps_payload_into_envelope() {
  let system = ActorSystem::new_empty().state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let probe = ProbeSender::new(messages);
  let target =
    ActorRefSenderSharedFactory::create_actor_ref_sender_shared(&BuiltinSpinSharedFactory::new(), Box::new(probe));
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 1, target, lifecycle, system);

  sender.send(AnyMessage::new(9_u32)).expect("send succeeds");

  let captured = messages_clone.lock().clone();
  assert_eq!(captured.len(), 1);
  let envelope = captured[0].payload().downcast_ref::<AdapterEnvelope>().expect("envelope");
  assert_eq!(envelope.type_id(), core::any::TypeId::of::<u32>());
}

#[test]
fn adapter_sender_rejects_when_lifecycle_stopped() {
  let system = ActorSystem::new_empty().state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  lifecycle.mark_stopped();
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let probe = ProbeSender::new(messages);
  let target =
    ActorRefSenderSharedFactory::create_actor_ref_sender_shared(&BuiltinSpinSharedFactory::new(), Box::new(probe));
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 2, target, lifecycle, system);

  let result = sender.send(AnyMessage::new(1_u8));
  assert!(matches!(result, Err(SendError::Closed(_))));
  assert!(messages_clone.lock().is_empty());
}
