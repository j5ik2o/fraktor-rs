use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use crate::core::{
  actor_prim::{
    Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender},
  },
  error::SendError,
  event_stream::{ActorRefEventStreamSubscriber, EventStreamEvent, EventStreamSubscriber},
  logging::{LogEvent, LogLevel},
  messaging::AnyMessageGeneric,
};

// Test sender that collects messages
struct CollectorSender {
  messages: NoStdMutex<alloc::vec::Vec<EventStreamEvent<NoStdToolbox>>>,
}

impl CollectorSender {
  fn new() -> Self {
    Self { messages: NoStdMutex::new(alloc::vec::Vec::new()) }
  }

  fn take_messages(&self) -> alloc::vec::Vec<EventStreamEvent<NoStdToolbox>> {
    self.messages.lock().drain(..).collect()
  }
}

impl ActorRefSender<NoStdToolbox> for CollectorSender {
  fn send(&self, message: AnyMessageGeneric<NoStdToolbox>) -> Result<(), SendError<NoStdToolbox>> {
    if let Some(event) = message.payload().downcast_ref::<EventStreamEvent<NoStdToolbox>>() {
      self.messages.lock().push(event.clone());
    }
    Ok(())
  }
}

#[test]
fn actor_ref_subscriber_forwards_events_to_actor() {
  let sender = ArcShared::new(CollectorSender::new());
  let actor_ref = ActorRefGeneric::new(Pid::new(1, 0), sender.clone());

  let subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  let event = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "test message".into(),
    Duration::from_millis(100),
    Some(Pid::new(1, 0)),
  ));

  subscriber.on_event(&event);

  let messages = sender.take_messages();
  assert_eq!(messages.len(), 1);
  assert!(matches!(messages[0], EventStreamEvent::Log(_)));
}

#[test]
fn actor_ref_subscriber_handles_multiple_events() {
  let sender = ArcShared::new(CollectorSender::new());
  let actor_ref = ActorRefGeneric::new(Pid::new(1, 0), sender.clone());

  let subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  for i in 0..10 {
    let event = EventStreamEvent::Log(LogEvent::new(
      LogLevel::Info,
      alloc::format!("message {}", i),
      Duration::from_millis(i as u64),
      Some(Pid::new(1, 0)),
    ));
    subscriber.on_event(&event);
  }

  let messages = sender.take_messages();
  assert_eq!(messages.len(), 10);
}

#[test]
fn actor_ref_returns_correct_reference() {
  let sender = ArcShared::new(CollectorSender::new());
  let actor_ref = ActorRefGeneric::new(Pid::new(1, 0), sender);

  let subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  assert_eq!(subscriber.actor_ref().pid(), actor_ref.pid());
}
