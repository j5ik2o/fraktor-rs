extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;

use fraktor_utils_core_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use crate::core::{
  actor_prim::Pid,
  dead_letter::{DeadLetter, DeadLetterReason},
  error::SendError,
  event_stream::{EventStream, EventStreamEvent, EventStreamSubscriber},
  logging::LogLevel,
  messaging::AnyMessage,
};

struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent<NoStdToolbox>> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn record_entry_stores_and_publishes() {
  let stream = ArcShared::new(EventStream::default());
  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  let deadletter = DeadLetter::with_default_capacity(stream.clone());
  let pid = Pid::new(1, 0);
  let message = AnyMessage::new("payload");
  deadletter.record_entry(message, DeadLetterReason::ExplicitRouting, Some(pid), Duration::from_millis(5));

  let entries = deadletter.entries();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].reason(), DeadLetterReason::ExplicitRouting);
  assert_eq!(entries[0].recipient(), Some(pid));

  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::DeadLetter(_))));
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn)));
}

#[test]
fn record_send_error_converts_reason_and_honours_capacity() {
  let stream = ArcShared::new(EventStream::default());
  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  let deadletter = DeadLetter::new(stream, 1);
  let pid = Pid::new(7, 0);
  let error = SendError::full(AnyMessage::new("first"));
  deadletter.record_send_error(Some(pid), &error, Duration::from_millis(1));
  deadletter.record_entry(
    AnyMessage::new("second"),
    DeadLetterReason::MailboxSuspended,
    Some(pid),
    Duration::from_millis(2),
  );

  let entries = deadletter.entries();
  assert_eq!(entries.len(), 1);
  assert!(matches!(entries[0].reason(), DeadLetterReason::MailboxSuspended));

  let events = subscriber_impl.events();
  assert!(events.iter().filter(|event| matches!(event, EventStreamEvent::DeadLetter(_))).count() >= 2);
}

#[test]
fn record_send_error_maps_timeout_reason() {
  let stream = ArcShared::new(EventStream::default());
  let deadletter = DeadLetter::with_default_capacity(stream);
  let pid = Pid::new(11, 0);
  let error = SendError::timeout(AnyMessage::new("delayed"));

  deadletter.record_send_error(Some(pid), &error, Duration::from_millis(3));

  let entries = deadletter.entries();
  assert!(
    entries.iter().any(|entry| entry.recipient() == Some(pid) && entry.reason() == DeadLetterReason::MailboxTimeout)
  );
}
