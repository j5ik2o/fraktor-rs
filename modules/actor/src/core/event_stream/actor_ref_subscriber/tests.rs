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
  messages: ArcShared<NoStdMutex<alloc::vec::Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl CollectorSender {
  fn new(messages: ArcShared<NoStdMutex<alloc::vec::Vec<EventStreamEvent<NoStdToolbox>>>>) -> Self {
    Self { messages }
  }
}

impl ActorRefSender<NoStdToolbox> for CollectorSender {
  fn send(
    &mut self,
    message: AnyMessageGeneric<NoStdToolbox>,
  ) -> Result<crate::core::actor_prim::actor_ref::SendOutcome, SendError<NoStdToolbox>> {
    if let Some(event) = message.payload().downcast_ref::<EventStreamEvent<NoStdToolbox>>() {
      self.messages.lock().push(event.clone());
    }
    Ok(crate::core::actor_prim::actor_ref::SendOutcome::Delivered)
  }
}

#[test]
fn actor_ref_subscriber_forwards_events_to_actor() {
  let messages = ArcShared::new(NoStdMutex::new(alloc::vec::Vec::new()));
  let messages_clone = messages.clone();
  let sender = CollectorSender::new(messages);
  let actor_ref = ActorRefGeneric::new(Pid::new(1, 0), sender);

  let mut subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  let event = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "test message".into(),
    Duration::from_millis(100),
    Some(Pid::new(1, 0)),
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
  let actor_ref = ActorRefGeneric::new(Pid::new(1, 0), sender);

  let mut subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  for i in 0..10 {
    let event = EventStreamEvent::Log(LogEvent::new(
      LogLevel::Info,
      alloc::format!("message {}", i),
      Duration::from_millis(i as u64),
      Some(Pid::new(1, 0)),
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
  let actor_ref = ActorRefGeneric::new(Pid::new(1, 0), sender);

  let subscriber = ActorRefEventStreamSubscriber::new(actor_ref.clone());

  assert_eq!(subscriber.actor_ref().pid(), actor_ref.pid());
}
