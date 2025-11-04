extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdMutex, NoStdToolbox,
  actor_prim::Pid,
  dead_letter::{DeadLetterGeneric, DeadLetterReason},
  error::SendError,
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber},
  logging::LogLevel,
  messaging::AnyMessageGeneric,
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
  let stream = ArcShared::new(EventStreamGeneric::default());
  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&stream, &subscriber);

  let deadletter = DeadLetterGeneric::with_default_capacity(stream.clone());
  let pid = Pid::new(1, 0);
  let message = AnyMessageGeneric::new("payload");
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
  let stream = ArcShared::new(EventStreamGeneric::default());
  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&stream, &subscriber);

  let deadletter = DeadLetterGeneric::new(stream, 1);
  let pid = Pid::new(7, 0);
  let error = SendError::full(AnyMessageGeneric::new("first"));
  deadletter.record_send_error(Some(pid), &error, Duration::from_millis(1));
  deadletter.record_entry(
    AnyMessageGeneric::new("second"),
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
