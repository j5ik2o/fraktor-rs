extern crate std;

use alloc::{borrow::ToOwned, collections::BTreeMap, format, string::String, vec::Vec};
use core::time::Duration;
use std::{
  fmt::Debug,
  sync::{Arc, Mutex},
};

use fraktor_actor_core_rs::core::kernel::event::{
  logging::{LogEvent, LogLevel},
  stream::{EventStreamEvent, EventStreamSubscriber},
};
use tracing::{
  Event, Level, Metadata, Subscriber,
  field::{Field, Visit},
  span::{Attributes, Id, Record},
  subscriber::with_default,
};

use super::{TracingLoggerSubscriber, duration_to_micros};

#[test]
fn forwards_log_events_to_tracing() {
  let collector = RecordingSubscriber::default();
  let shared = collector.clone();
  with_default(shared, || {
    let mut subscriber = TracingLoggerSubscriber::new(LogLevel::Trace);
    let log = LogEvent::new(LogLevel::Info, String::from("hello"), Duration::from_micros(42), None, None);
    subscriber.on_event(&EventStreamEvent::Log(log));
  });

  let events = collector.events();
  assert_eq!(events.len(), 1);
  let event = &events[0];
  assert_eq!(event.level, Level::INFO);
  assert_eq!(event.target, TracingLoggerSubscriber::DEFAULT_TARGET);
  assert_eq!(event.message, "hello");
  assert_eq!(event.timestamp_micros, Some(42));
  assert_eq!(event.origin, Some(String::from("n/a")));
  assert_eq!(event.logger_name, Some(String::from("n/a")));
  assert_eq!(event.marker_name, Some(String::from("n/a")));
  assert_eq!(event.marker_properties, Some(String::from("{}")));
  assert_eq!(event.mdc, Some(String::from("{}")));
}

#[test]
fn filters_events_below_threshold() {
  let collector = RecordingSubscriber::default();
  let shared = collector.clone();
  with_default(shared, || {
    let mut subscriber = TracingLoggerSubscriber::new(LogLevel::Warn);
    let info = LogEvent::new(LogLevel::Info, String::from("info"), Duration::ZERO, None, None);
    subscriber.on_event(&EventStreamEvent::Log(info));
    let warn = LogEvent::new(LogLevel::Warn, String::from("warn"), Duration::ZERO, None, None);
    subscriber.on_event(&EventStreamEvent::Log(warn));
  });

  let events = collector.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].message, "warn");
}

#[test]
fn forwards_structured_marker_and_mdc_fields_to_tracing() {
  let collector = RecordingSubscriber::default();
  let shared = collector.clone();
  with_default(shared, || {
    let mut subscriber = TracingLoggerSubscriber::new(LogLevel::Trace);
    let marker_properties = BTreeMap::from([(String::from("pekkoMessageClass"), String::from("ExampleMessage"))]);
    let mdc = BTreeMap::from([(String::from("iam"), String::from("the one who knocks"))]);
    let log = LogEvent::new(
      LogLevel::Warn,
      String::from("structured"),
      Duration::from_micros(11),
      None,
      Some(String::from("classic.logging")),
    )
    .with_marker("pekkoDeadLetter", marker_properties)
    .with_mdc(mdc);
    subscriber.on_event(&EventStreamEvent::Log(log));
  });

  let events = collector.events();
  assert_eq!(events.len(), 1);
  let event = &events[0];
  assert_eq!(event.logger_name, Some(String::from("classic.logging")));
  assert_eq!(event.marker_name, Some(String::from("pekkoDeadLetter")));
  assert_eq!(event.marker_properties, Some(String::from("{\"pekkoMessageClass\": \"ExampleMessage\"}")));
  assert_eq!(event.mdc, Some(String::from("{\"iam\": \"the one who knocks\"}")));
}

#[test]
fn forwards_trace_debug_and_error_levels_to_tracing() {
  let collector = RecordingSubscriber::default();
  let shared = collector.clone();
  with_default(shared, || {
    let mut subscriber = TracingLoggerSubscriber::new(LogLevel::Trace);
    for (level, message) in [(LogLevel::Trace, "trace"), (LogLevel::Debug, "debug"), (LogLevel::Error, "error")] {
      subscriber.on_event(&EventStreamEvent::Log(LogEvent::new(
        level,
        String::from(message),
        Duration::ZERO,
        None,
        None,
      )));
    }
  });

  let events = collector.events();
  assert_eq!(events.iter().map(|event| event.level).collect::<Vec<_>>(), vec![
    Level::TRACE,
    Level::DEBUG,
    Level::ERROR
  ]);
  assert_eq!(events.iter().map(|event| event.message.as_str()).collect::<Vec<_>>(), vec!["trace", "debug", "error"]);
}

#[test]
fn duration_to_micros_saturates_at_u64_max() {
  assert_eq!(duration_to_micros(Duration::from_secs(u64::MAX)), u64::MAX);
}

#[derive(Clone, Default)]
struct RecordingSubscriber {
  events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl RecordingSubscriber {
  fn events(&self) -> Vec<CapturedEvent> {
    self.events.lock().expect("lock").clone()
  }
}

impl Subscriber for RecordingSubscriber {
  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn new_span(&self, _: &Attributes<'_>) -> Id {
    Id::from_u64(1)
  }

  fn record(&self, _: &Id, _: &Record<'_>) {}

  fn record_follows_from(&self, _: &Id, _: &Id) {}

  fn event(&self, event: &Event<'_>) {
    let metadata = event.metadata();
    let mut visitor = EventVisitor::default();
    event.record(&mut visitor);
    let captured = CapturedEvent {
      level:             *metadata.level(),
      target:            metadata.target().to_owned(),
      message:           visitor.message.unwrap_or_default(),
      origin:            visitor.origin.or_else(|| Some(String::from("n/a"))),
      logger_name:       visitor.logger_name.or_else(|| Some(String::from("n/a"))),
      marker_name:       visitor.marker_name.or_else(|| Some(String::from("n/a"))),
      marker_properties: visitor.marker_properties.or_else(|| Some(String::from("{}"))),
      mdc:               visitor.mdc.or_else(|| Some(String::from("{}"))),
      timestamp_micros:  visitor.timestamp_micros,
    };
    self.events.lock().expect("lock").push(captured);
  }

  fn enter(&self, _: &Id) {}

  fn exit(&self, _: &Id) {}
}

#[derive(Clone, Debug)]
struct CapturedEvent {
  level:             Level,
  target:            String,
  message:           String,
  origin:            Option<String>,
  logger_name:       Option<String>,
  marker_name:       Option<String>,
  marker_properties: Option<String>,
  mdc:               Option<String>,
  timestamp_micros:  Option<u64>,
}

impl Default for CapturedEvent {
  fn default() -> Self {
    Self {
      level:             Level::INFO,
      target:            String::new(),
      message:           String::new(),
      origin:            None,
      logger_name:       None,
      marker_name:       None,
      marker_properties: None,
      mdc:               None,
      timestamp_micros:  None,
    }
  }
}

#[derive(Default)]
struct EventVisitor {
  message:           Option<String>,
  origin:            Option<String>,
  logger_name:       Option<String>,
  marker_name:       Option<String>,
  marker_properties: Option<String>,
  mdc:               Option<String>,
  timestamp_micros:  Option<u64>,
}

impl Visit for EventVisitor {
  fn record_str(&mut self, field: &Field, value: &str) {
    match field.name() {
      | "message" => self.message = Some(value.to_owned()),
      | "origin" => self.origin = Some(value.to_owned()),
      | "logger_name" => self.logger_name = Some(value.to_owned()),
      | "marker_name" => self.marker_name = Some(value.to_owned()),
      | _ => {},
    }
  }

  fn record_u64(&mut self, field: &Field, value: u64) {
    if field.name() == "timestamp_micros" {
      self.timestamp_micros = Some(value);
    }
  }

  fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
    if field.name() == "message" && self.message.is_none() {
      self.message = Some(format_value(value));
    } else if field.name() == "origin" && self.origin.is_none() {
      self.origin = Some(format_value(value));
    } else if field.name() == "marker_properties" {
      self.marker_properties = Some(format!("{value:?}"));
    } else if field.name() == "mdc" {
      self.mdc = Some(format!("{value:?}"));
    }
  }
}

fn format_value(value: &dyn Debug) -> String {
  let rendered = format!("{value:?}");
  rendered.trim_matches('"').to_owned()
}
