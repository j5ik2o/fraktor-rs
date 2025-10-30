use alloc::vec::Vec;
use core::mem;

use cellactor_utils_core_rs::sync::{ArcShared, async_mutex_like::SpinAsyncMutex};

use crate::{
  AnyOwnedMessage, Pid, SendError,
  actor_ref::{ActorRef, ActorRefSender},
};

struct RecordingSender {
  messages: SpinAsyncMutex<Vec<AnyOwnedMessage>>,
}

impl RecordingSender {
  fn new() -> Self {
    Self { messages: SpinAsyncMutex::new(Vec::new()) }
  }

  fn drain(&self) -> Vec<AnyOwnedMessage> {
    let mut guard = self.messages.lock();
    let mut collected = Vec::new();
    mem::swap(&mut *guard, &mut collected);
    collected
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    self.messages.lock().push(message);
    Ok(())
  }
}

struct FailingSender;

impl ActorRefSender for FailingSender {
  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    Err(SendError::Full(message))
  }
}

#[test]
fn tell_enqueues_message() {
  let storage = ArcShared::new(RecordingSender::new());
  let handle = ActorRef::new(Pid::new(1, 0), storage.clone());

  let payload = AnyOwnedMessage::new(42_u32);
  assert!(handle.tell(payload.clone()).is_ok());

  let mut drained = storage.drain();
  assert_eq!(drained.len(), 1);
  let envelope = drained.pop().unwrap();
  let borrowed = envelope.as_any();
  assert_eq!(borrowed.downcast_ref::<u32>(), Some(&42));
}

#[test]
fn tell_propagates_error() {
  let failing_sender = ArcShared::new(FailingSender);
  let handle = ActorRef::new(Pid::new(2, 0), failing_sender);
  let payload = AnyOwnedMessage::new(7_u8);

  let result = handle.tell(payload);
  assert!(matches!(result, Err(SendError::Full(_))));
}

#[test]
fn ask_completes_future_on_reply() {
  let storage = ArcShared::new(RecordingSender::new());
  let handle = ActorRef::new(Pid::new(3, 0), storage.clone());

  let response = handle.ask(AnyOwnedMessage::new("ping")).expect("ask should succeed");
  let (reply_to, future) = response.into_parts();

  let mut drained = storage.drain();
  assert_eq!(drained.len(), 1);
  let envelope = drained.pop().unwrap();
  let borrowed = envelope.as_any();
  assert!(borrowed.downcast_ref::<&str>().is_some());
  let reply = borrowed.reply_to().expect("reply_to must be set").clone();

  reply.tell(AnyOwnedMessage::new("pong")).expect("reply should succeed");

  let result = future.try_take().expect("future must contain reply");
  let borrowed = result.as_any();
  assert_eq!(borrowed.downcast_ref::<&str>(), Some(&"pong"));

  // reply_to returned by ask should be the same handle.
  assert_eq!(reply.pid(), reply_to.pid());
}
