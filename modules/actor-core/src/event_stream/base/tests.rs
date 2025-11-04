extern crate alloc;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdMutex, NoStdToolbox,
  actor_prim::Pid,
  event_stream::{EventStream, EventStreamEvent, EventStreamSubscriber},
  lifecycle::{LifecycleEvent, LifecycleStage},
  logging::{LogEvent, LogLevel},
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

impl EventStreamSubscriber<NoStdToolbox> for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn event_stream_replays_buffer_for_new_subscribers() {
  let stream = ArcShared::new(EventStream::default());

  let log = LogEvent::new(LogLevel::Info, String::from("boot"), Duration::from_millis(1), None);
  stream.publish(&EventStreamEvent::Log(log));

  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  let lifecycle =
    LifecycleEvent::new(Pid::new(1, 0), None, String::from("actor"), LifecycleStage::Started, Duration::from_millis(2));
  stream.publish(&EventStreamEvent::Lifecycle(lifecycle));

  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::Log(_))));
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::Lifecycle(_))));
}

#[test]
fn capacity_limits_buffer_size() {
  let stream = ArcShared::new(EventStream::with_capacity(1));

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("first"),
    Duration::from_millis(1),
    None,
  )));
  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("second"),
    Duration::from_millis(2),
    None,
  )));

  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  let events = subscriber_impl.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(&events[0], EventStreamEvent::Log(event) if event.message() == "second"));
}

#[test]
fn unsubscribe_removes_subscriber() {
  let stream = ArcShared::new(EventStream::default());
  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let subscription = EventStream::subscribe_arc(&stream, &subscriber);

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("before unsubscribe"),
    Duration::from_millis(1),
    None,
  )));

  stream.unsubscribe(subscription.id());

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("after unsubscribe"),
    Duration::from_millis(2),
    None,
  )));

  let events = subscriber_impl.events();
  assert!(
    events.iter().any(|event| matches!(event, EventStreamEvent::Log(event) if event.message() == "before unsubscribe"))
  );
  assert!(
    !events.iter().any(|event| matches!(event, EventStreamEvent::Log(event) if event.message() == "after unsubscribe"))
  );
}

#[test]
fn default_creates_stream_with_default_capacity() {
  let stream = EventStream::default();
  let _ = stream;
}

#[test]
fn with_capacity_creates_stream_with_specified_capacity() {
  let stream = EventStream::with_capacity(100);
  let _ = stream;
}
