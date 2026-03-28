use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::{
  kernel::{
    actor::{
      Pid,
      actor_ref::{ActorRefSender, ActorRefSenderShared},
    },
    error::SendError,
    messaging::AnyMessage,
    system::ActorSystem,
  },
  typed::message_adapter::{AdapterEnvelope, AdapterLifecycleState, AdapterRefSender},
};

struct ProbeSender {
  messages: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
}

impl ProbeSender {
  fn new(messages: ArcShared<NoStdMutex<Vec<AnyMessage>>>) -> Self {
    Self { messages }
  }
}

impl ActorRefSender for ProbeSender {
  fn send(&mut self, message: AnyMessage) -> Result<crate::core::kernel::actor::actor_ref::SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(crate::core::kernel::actor::actor_ref::SendOutcome::Delivered)
  }
}

#[test]
fn adapter_sender_wraps_payload_into_envelope() {
  let system = ActorSystem::new_empty().state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  let messages = ArcShared::new(NoStdMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let probe = ProbeSender::new(messages);
  let target = ActorRefSenderShared::new(probe);
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
  let messages = ArcShared::new(NoStdMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let probe = ProbeSender::new(messages);
  let target = ActorRefSenderShared::new(probe);
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 2, target, lifecycle, system);

  let result = sender.send(AnyMessage::new(1_u8));
  assert!(matches!(result, Err(SendError::Closed(_))));
  assert!(messages_clone.lock().is_empty());
}
