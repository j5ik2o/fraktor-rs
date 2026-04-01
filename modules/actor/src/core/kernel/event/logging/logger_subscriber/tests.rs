use alloc::{boxed::Box, string::String, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::kernel::{
  actor::actor_ref::dead_letter::DeadLetterEntry,
  event::{
    logging::{LogEvent, LogLevel, LoggerSubscriber, LoggerWriter},
    stream::{EventStreamEvent, EventStreamSubscriber},
  },
};

struct TestWriter {
  logs: ArcShared<NoStdMutex<Vec<String>>>,
}

impl TestWriter {
  fn new() -> (Self, ArcShared<NoStdMutex<Vec<String>>>) {
    let logs = ArcShared::new(NoStdMutex::new(Vec::new()));
    (Self { logs: logs.clone() }, logs)
  }
}

impl LoggerWriter for TestWriter {
  fn write(&mut self, event: &LogEvent) {
    self.logs.lock().push(event.message().into());
  }
}

#[test]
fn new_creates_subscriber() {
  let (writer, _) = TestWriter::new();
  let subscriber = LoggerSubscriber::new(LogLevel::Info, Box::new(writer));
  assert_eq!(subscriber.level(), LogLevel::Info);
}

#[test]
fn level_returns_configured_level() {
  let (writer, _) = TestWriter::new();
  let subscriber = LoggerSubscriber::new(LogLevel::Debug, Box::new(writer));
  assert_eq!(subscriber.level(), LogLevel::Debug);
}

#[test]
fn on_event_filters_by_level() {
  let (writer, logs) = TestWriter::new();
  let mut subscriber = LoggerSubscriber::new(LogLevel::Warn, Box::new(writer));

  let debug_event = LogEvent::new(LogLevel::Debug, String::from("debug"), Duration::ZERO, None, None);
  let warn_event = LogEvent::new(LogLevel::Warn, String::from("warn"), Duration::ZERO, None, None);
  let error_event = LogEvent::new(LogLevel::Error, String::from("error"), Duration::ZERO, None, None);

  subscriber.on_event(&EventStreamEvent::Log(debug_event));
  subscriber.on_event(&EventStreamEvent::Log(warn_event));
  subscriber.on_event(&EventStreamEvent::Log(error_event));

  let recorded = logs.lock();
  assert_eq!(recorded.len(), 2);
  assert_eq!(recorded[0], "warn");
  assert_eq!(recorded[1], "error");
}

#[test]
fn on_event_writes_matching_logs() {
  let (writer, logs) = TestWriter::new();
  let mut subscriber = LoggerSubscriber::new(LogLevel::Info, Box::new(writer));

  let event = LogEvent::new(LogLevel::Info, String::from("test message"), Duration::ZERO, None, None);
  subscriber.on_event(&EventStreamEvent::Log(event));

  let recorded = logs.lock();
  assert_eq!(recorded.len(), 1);
  assert_eq!(recorded[0], "test message");
}

#[test]
fn on_event_ignores_non_log_events() {
  let (writer, logs) = TestWriter::new();
  let mut subscriber = LoggerSubscriber::new(LogLevel::Info, Box::new(writer));

  subscriber.on_event(&EventStreamEvent::DeadLetter(DeadLetterEntry::new(
    crate::core::kernel::actor::messaging::AnyMessage::new(()),
    crate::core::kernel::actor::actor_ref::dead_letter::DeadLetterReason::MissingRecipient,
    None,
    Duration::ZERO,
  )));

  let recorded = logs.lock();
  assert_eq!(recorded.len(), 0);
}
