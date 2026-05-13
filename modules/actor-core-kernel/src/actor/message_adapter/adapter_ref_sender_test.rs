use alloc::{boxed::Box, vec::Vec};

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::AdapterRefSender;
use crate::{
  actor::{
    Pid,
    actor_ref::{ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    message_adapter::AdapterLifecycleState,
    messaging::{AnyMessage, NotInfluenceReceiveTimeout},
  },
  system::state::{SystemStateShared, system_state::SystemState},
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

struct FailingSender;

impl ActorRefSender for FailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::no_recipient(message))
  }
}

fn empty_system_state() -> SystemStateShared {
  SystemStateShared::new(SystemState::new())
}

#[test]
fn adapter_sender_delegates_wrapped_message_to_target() {
  let system = empty_system_state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let target = ActorRefSenderShared::new(Box::new(ProbeSender::new(messages)));
  let mut sender = AdapterRefSender::new(
    Pid::new(1, 0),
    1,
    target,
    lifecycle,
    system,
    Box::new(|_message| AnyMessage::new("wrapped")),
  );

  sender.send(AnyMessage::new(9_u32)).expect("send succeeds");

  let captured = messages_clone.lock().clone();
  assert_eq!(captured.len(), 1);
  assert_eq!(captured[0].payload().downcast_ref::<&str>(), Some(&"wrapped"));
}

#[test]
fn adapter_sender_rejects_when_lifecycle_stopped() {
  let system = empty_system_state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  lifecycle.mark_stopped();
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let messages_clone = messages.clone();
  let target = ActorRefSenderShared::new(Box::new(ProbeSender::new(messages)));
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 2, target, lifecycle, system, Box::new(|message| message));

  let result = sender.send(AnyMessage::not_influence(AdapterTick));

  assert!(matches!(result, Err(SendError::Closed(_))));
  assert!(messages_clone.lock().is_empty());
}

#[test]
fn adapter_sender_returns_target_send_error() {
  let system = empty_system_state();
  let lifecycle = ArcShared::new(AdapterLifecycleState::new());
  let target = ActorRefSenderShared::new(Box::new(FailingSender));
  let mut sender = AdapterRefSender::new(Pid::new(1, 0), 3, target, lifecycle, system, Box::new(|message| message));

  let result = sender.send(AnyMessage::new(9_u32));

  assert!(matches!(result, Err(SendError::NoRecipient(_))));
}
