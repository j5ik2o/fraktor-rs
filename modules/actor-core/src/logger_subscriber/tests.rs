#![cfg(feature = "std")]

extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  EventStream, EventStreamEvent, EventStreamSubscriber, LifecycleEvent, LifecycleStage, LogEvent, LogLevel,
  LoggerSubscriber, LoggerWriter, NoStdMutex, Pid,
};

struct RecordingWriter {
  events: ArcShared<NoStdMutex<Vec<LogEvent>>>,
}

impl RecordingWriter {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<LogEvent> {
    self.events.lock().clone()
  }
}

impl LoggerWriter for RecordingWriter {
  fn write(&self, event: &LogEvent) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn forwards_events_at_or_above_threshold() {
  let stream = ArcShared::new(EventStream::default());
  let writer = ArcShared::new(RecordingWriter::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(LoggerSubscriber::new(LogLevel::Info, writer.clone()));
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Debug,
    alloc::string::String::from("debug message"),
    Duration::from_millis(1),
    None,
  )));
  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Warn,
    alloc::string::String::from("warn message"),
    Duration::from_millis(2),
    None,
  )));

  let events = writer.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].message(), "warn message");
  assert_eq!(events[0].level(), LogLevel::Warn);
}

#[test]
fn ignores_non_log_events() {
  let stream = ArcShared::new(EventStream::default());
  let writer = ArcShared::new(RecordingWriter::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber> =
    ArcShared::new(LoggerSubscriber::new(LogLevel::Trace, writer.clone()));
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  let lifecycle = LifecycleEvent::new(
    Pid::new(1, 0),
    None,
    alloc::string::String::from("actor"),
    LifecycleStage::Started,
    Duration::from_millis(3),
  );
  stream.publish(&EventStreamEvent::Lifecycle(lifecycle));

  assert!(writer.events().is_empty());
}
