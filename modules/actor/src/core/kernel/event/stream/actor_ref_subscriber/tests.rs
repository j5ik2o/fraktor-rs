use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender},
    error::SendError,
    messaging::AnyMessage,
  },
  event::{
    logging::{LogEvent, LogLevel},
    stream::{ActorRefEventStreamSubscriber, EventStreamEvent, EventStreamSubscriber},
  },
};

// Test sender that collects messages
struct CollectorSender {
  messages: ArcShared<NoStdMutex<alloc::vec::Vec<EventStreamEvent>>>,
}

impl CollectorSender {
  fn new(messages: ArcShared<NoStdMutex<alloc::vec::Vec<EventStreamEvent>>>) -> Self {
    Self { messages }
  }
}

impl ActorRefSender for CollectorSender {
  fn send(&mut self, message: AnyMessage) -> Result<crate::core::kernel::actor::actor_ref::SendOutcome, SendError> {
    if let Some(event) = message.payload().downcast_ref::<EventStreamEvent>() {
      self.messages.lock().push(event.clone());
    }
    Ok(crate::core::kernel::actor::actor_ref::SendOutcome::Delivered)
  }
}

#[test]
fn actor_ref_subscriber_forwards_events_to_actor() {
  let messages = ArcShared::new(NoStdMutex::new(alloc::vec::Vec::new()));
  let messages_clone = messages.clone();
  let sender = CollectorSender::new(messages);
  let actor_ref = ActorRef::new(Pid::new(1, 0), sender);

  let mut subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  let event = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "test message".into(),
    Duration::from_millis(100),
    Some(Pid::new(1, 0)),
    None,
  ));

  subscriber.on_event(&event);

  let captured: alloc::vec::Vec<_> = messages_clone.lock().drain(..).collect();
  assert_eq!(captured.len(), 1);
  assert!(matches!(captured[0], EventStreamEvent::Log(_)));
}

#[test]
fn actor_ref_subscriber_handles_multiple_events() {
  let messages = ArcShared::new(NoStdMutex::new(alloc::vec::Vec::new()));
  let messages_clone = messages.clone();
  let sender = CollectorSender::new(messages);
  let actor_ref = ActorRef::new(Pid::new(1, 0), sender);

  let mut subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  for i in 0..10 {
    let event = EventStreamEvent::Log(LogEvent::new(
      LogLevel::Info,
      alloc::format!("message {}", i),
      Duration::from_millis(i as u64),
      Some(Pid::new(1, 0)),
      None,
    ));
    subscriber.on_event(&event);
  }

  let captured: alloc::vec::Vec<_> = messages_clone.lock().drain(..).collect();
  assert_eq!(captured.len(), 10);
}

#[test]
fn actor_ref_returns_correct_reference() {
  let messages = ArcShared::new(NoStdMutex::new(alloc::vec::Vec::new()));
  let sender = CollectorSender::new(messages);
  let actor_ref = ActorRef::new(Pid::new(1, 0), sender);

  let subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  assert_eq!(subscriber.actor_ref().pid(), actor_ref.pid());
}
