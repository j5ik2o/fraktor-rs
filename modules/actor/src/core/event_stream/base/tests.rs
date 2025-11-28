extern crate alloc;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::EventStream;
use crate::core::{
  actor_prim::Pid,
  event_stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  lifecycle::{LifecycleEvent, LifecycleStage},
  logging::{LogEvent, LogLevel},
  messaging::AnyMessage,
};

struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>>>>,
}

impl RecordingSubscriber {
  fn new(
    events: ArcShared<NoStdMutex<Vec<EventStreamEvent<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>>>>,
  ) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn event_stream_replays_buffer_for_new_subscribers() {
  let stream = ArcShared::new(EventStream::default());

  let log = LogEvent::new(LogLevel::Info, String::from("boot"), Duration::from_millis(1), None);
  stream.publish(&EventStreamEvent::Log(log));

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  let lifecycle =
    LifecycleEvent::new(Pid::new(1, 0), None, String::from("actor"), LifecycleStage::Started, Duration::from_millis(2));
  stream.publish(&EventStreamEvent::Lifecycle(lifecycle));

  let events = events.lock().clone();
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

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  let events = events.lock().clone();
  assert_eq!(events.len(), 1);
  assert!(matches!(&events[0], EventStreamEvent::Log(event) if event.message() == "second"));
}

#[test]
fn extension_events_are_buffered_and_delivered() {
  let stream = ArcShared::new(EventStream::with_capacity(4));

  // publish before subscription to ensure replay works
  stream.publish(&EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(String::from("startup")),
  });

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  stream.publish(&EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(String::from("shutdown")),
  });

  let events = events.lock().clone();
  assert_eq!(events.len(), 2);
  assert!(events.iter().any(|event| match event {
    | EventStreamEvent::Extension { name, payload } => {
      name == "cluster" && payload.payload().downcast_ref::<String>().map(|s| s == "startup").unwrap_or(false)
    },
    | _ => false,
  }));
  assert!(events.iter().any(|event| match event {
    | EventStreamEvent::Extension { name, payload } => {
      name == "cluster" && payload.payload().downcast_ref::<String>().map(|s| s == "shutdown").unwrap_or(false)
    },
    | _ => false,
  }));
}

#[test]
fn unsubscribe_removes_subscriber() {
  let stream = ArcShared::new(EventStream::default());
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
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

  let events = events.lock().clone();
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
