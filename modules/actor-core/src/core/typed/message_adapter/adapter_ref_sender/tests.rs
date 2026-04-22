use alloc::vec::Vec;
use core::any::TypeId;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::{
  kernel::{
    actor::{
      Pid,
      actor_ref::{ActorRefSender, ActorRefSenderShared},
      error::SendError,
      messaging::{AnyMessage, NotInfluenceReceiveTimeout},
    },
    system::ActorSystem,
  },
  typed::message_adapter::{AdapterEnvelope, AdapterLifecycleState, AdapterRefSender, adapter_ref_sender::SendOutcome},
};

struct AdapterTick;

impl NotInfluenceReceiveTimeout for AdapterTick {}

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
  let target = ActorRefSenderShared::new(Box::new(probe));
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 1, target, lifecycle, system);

  sender.send(AnyMessage::new(9_u32)).expect("send succeeds");

  let captured = messages_clone.lock().clone();
  assert_eq!(captured.len(), 1);
  let envelope = captured[0].payload().downcast_ref::<AdapterEnvelope>().expect("envelope");
  assert_eq!(envelope.type_id(), TypeId::of::<u32>());
}

#[test]
fn adapter_sender_preserves_not_influence_receive_timeout_flag() {
  let system = ActorSystem::new_empty().state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let probe = ProbeSender::new(messages);
  let target = ActorRefSenderShared::new(Box::new(probe));
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 3, target, lifecycle, system);

  sender.send(AnyMessage::not_influence(AdapterTick)).expect("send succeeds");

  let captured = messages_clone.lock().clone();
  assert_eq!(captured.len(), 1);
  assert!(
    captured[0].is_not_influence_receive_timeout(),
    "adapter boundary must preserve NotInfluenceReceiveTimeout marker flag"
  );
  assert!(!captured[0].is_control(), "non-control input must remain non-control");
}

#[test]
fn adapter_sender_preserves_control_flag_without_not_influence() {
  let system = ActorSystem::new_empty().state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let probe = ProbeSender::new(messages);
  let target = ActorRefSenderShared::new(Box::new(probe));
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 4, target, lifecycle, system);

  sender.send(AnyMessage::control(7_u32)).expect("send succeeds");

  let captured = messages_clone.lock().clone();
  assert_eq!(captured.len(), 1);
  assert!(captured[0].is_control(), "control flag must survive adapter boundary");
  assert!(
    !captured[0].is_not_influence_receive_timeout(),
    "control-only message must not spuriously acquire NotInfluenceReceiveTimeout"
  );
}

#[test]
fn adapter_sender_rejects_when_lifecycle_stopped() {
  let system = ActorSystem::new_empty().state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  lifecycle.mark_stopped();
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let probe = ProbeSender::new(messages);
  let target = ActorRefSenderShared::new(Box::new(probe));
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 2, target, lifecycle, system);

  let result = sender.send(AnyMessage::new(1_u8));
  assert!(matches!(result, Err(SendError::Closed(_))));
  assert!(messages_clone.lock().is_empty());
}
