use alloc::{string::String, vec::Vec};
use core::time::Duration;

use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

use crate::{
  NoStdToolbox,
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  logging::{LogEvent, LogLevel, LoggerSubscriber, LoggerWriter},
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
  fn write(&self, event: &LogEvent) {
    self.logs.lock().push(event.message().into());
  }
}

#[test]
fn new_creates_subscriber() {
  let (writer, _) = TestWriter::new();
  let subscriber = LoggerSubscriber::new(LogLevel::Info, ArcShared::new(writer));
  assert_eq!(subscriber.level(), LogLevel::Info);
}

#[test]
fn level_returns_configured_level() {
  let (writer, _) = TestWriter::new();
  let subscriber = LoggerSubscriber::new(LogLevel::Debug, ArcShared::new(writer));
  assert_eq!(subscriber.level(), LogLevel::Debug);
}

#[test]
fn on_event_filters_by_level() {
  let (writer, logs) = TestWriter::new();
  let subscriber = LoggerSubscriber::new(LogLevel::Warn, ArcShared::new(writer));

  let debug_event = LogEvent::new(LogLevel::Debug, String::from("debug"), Duration::ZERO, None);
  let warn_event = LogEvent::new(LogLevel::Warn, String::from("warn"), Duration::ZERO, None);
  let error_event = LogEvent::new(LogLevel::Error, String::from("error"), Duration::ZERO, None);

  subscriber.on_event(&EventStreamEvent::<NoStdToolbox>::Log(debug_event));
  subscriber.on_event(&EventStreamEvent::<NoStdToolbox>::Log(warn_event));
  subscriber.on_event(&EventStreamEvent::<NoStdToolbox>::Log(error_event));

  let recorded = logs.lock();
  assert_eq!(recorded.len(), 2);
  assert_eq!(recorded[0], "warn");
  assert_eq!(recorded[1], "error");
}

#[test]
fn on_event_writes_matching_logs() {
  let (writer, logs) = TestWriter::new();
  let subscriber = LoggerSubscriber::new(LogLevel::Info, ArcShared::new(writer));

  let event = LogEvent::new(LogLevel::Info, String::from("test message"), Duration::ZERO, None);
  subscriber.on_event(&EventStreamEvent::<NoStdToolbox>::Log(event));

  let recorded = logs.lock();
  assert_eq!(recorded.len(), 1);
  assert_eq!(recorded[0], "test message");
}

#[test]
fn on_event_ignores_non_log_events() {
  let (writer, logs) = TestWriter::new();
  let subscriber = LoggerSubscriber::new(LogLevel::Info, ArcShared::new(writer));

  subscriber.on_event(&EventStreamEvent::<NoStdToolbox>::DeadLetter(crate::dead_letter::DeadLetterEntryGeneric::new(
    crate::messaging::AnyMessageGeneric::new(()),
    crate::dead_letter::DeadLetterReason::MissingRecipient,
    None,
    Duration::ZERO,
  )));

  let recorded = logs.lock();
  assert_eq!(recorded.len(), 0);
}
