use alloc::vec::Vec;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use crate::core::{
  actor_prim::{Pid, actor_ref::ActorRefSender},
  error::SendError,
  messaging::AnyMessageGeneric,
  system::SystemStateGeneric,
  typed::message_adapter::{AdapterEnvelope, AdapterLifecycleState, AdapterRefHandleId, AdapterRefSender},
};

struct ProbeSender {
  messages: NoStdMutex<Vec<AnyMessageGeneric<NoStdToolbox>>>,
}

impl ProbeSender {
  fn new() -> Self {
    Self { messages: NoStdMutex::new(Vec::new()) }
  }
}

impl ActorRefSender<NoStdToolbox> for ProbeSender {
  fn send(&self, message: AnyMessageGeneric<NoStdToolbox>) -> Result<(), SendError<NoStdToolbox>> {
    self.messages.lock().push(message);
    Ok(())
  }
}

impl ProbeSender {
  fn messages(&self) -> Vec<AnyMessageGeneric<NoStdToolbox>> {
    self.messages.lock().clone()
  }
}

#[test]
fn adapter_sender_wraps_payload_into_envelope() {
  let system = ArcShared::new(SystemStateGeneric::new());
  let lifecycle = ArcShared::new(AdapterLifecycleState::new(system.clone(), Pid::new(1, 0)));
  let probe = ArcShared::new(ProbeSender::new());
  let target: ArcShared<dyn ActorRefSender<NoStdToolbox>> = probe.clone();
  let sender = AdapterRefSender::new(Pid::new(1, 0), AdapterRefHandleId::new(1), target, lifecycle, system);

  sender.send(AnyMessageGeneric::new(9_u32)).expect("send succeeds");

  let messages = probe.messages();
  assert_eq!(messages.len(), 1);
  let envelope = messages[0].payload().downcast_ref::<AdapterEnvelope<NoStdToolbox>>().expect("envelope");
  assert_eq!(envelope.type_id(), core::any::TypeId::of::<u32>());
}

#[test]
fn adapter_sender_rejects_when_lifecycle_stopped() {
  let system = ArcShared::new(SystemStateGeneric::new());
  let lifecycle = ArcShared::new(AdapterLifecycleState::new(system.clone(), Pid::new(1, 0)));
  lifecycle.mark_stopped();
  let probe = ArcShared::new(ProbeSender::new());
  let target: ArcShared<dyn ActorRefSender<NoStdToolbox>> = probe.clone();
  let sender = AdapterRefSender::new(Pid::new(1, 0), AdapterRefHandleId::new(2), target, lifecycle, system);

  let result = sender.send(AnyMessageGeneric::new(1_u8));
  assert!(matches!(result, Err(SendError::Closed(_))));
  assert!(probe.messages().is_empty());
}
