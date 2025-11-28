extern crate std;

use alloc::{borrow::ToOwned, format, string::String, vec::Vec};
use core::time::Duration;
use std::{
  fmt,
  sync::{Arc, Mutex},
};

use tracing::{
  Event, Level, Metadata, Subscriber,
  field::{Field, Visit},
  span::{Attributes, Id, Record},
  subscriber::with_default,
};

use super::TracingLoggerSubscriber;
use crate::{
  core::logging::{LogEvent, LogLevel},
  std::event_stream::{EventStreamEvent, EventStreamSubscriber},
};

#[test]
fn forwards_log_events_to_tracing() {
  let collector = RecordingSubscriber::default();
  let shared = collector.clone();
  with_default(shared, || {
    let mut subscriber = TracingLoggerSubscriber::new(LogLevel::Trace);
    let log = LogEvent::new(LogLevel::Info, String::from("hello"), Duration::from_micros(42), None);
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
}

#[test]
fn filters_events_below_threshold() {
  let collector = RecordingSubscriber::default();
  let shared = collector.clone();
  with_default(shared, || {
    let mut subscriber = TracingLoggerSubscriber::new(LogLevel::Warn);
    let info = LogEvent::new(LogLevel::Info, String::from("info"), Duration::ZERO, None);
    subscriber.on_event(&EventStreamEvent::Log(info));
    let warn = LogEvent::new(LogLevel::Warn, String::from("warn"), Duration::ZERO, None);
    subscriber.on_event(&EventStreamEvent::Log(warn));
  });

  let events = collector.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].message, "warn");
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
    Id::from_u64(0)
  }

  fn record(&self, _: &Id, _: &Record<'_>) {}

  fn record_follows_from(&self, _: &Id, _: &Id) {}

  fn event(&self, event: &Event<'_>) {
    let metadata = event.metadata();
    let mut visitor = EventVisitor::default();
    event.record(&mut visitor);
    let captured = CapturedEvent {
      level:            *metadata.level(),
      target:           metadata.target().to_owned(),
      message:          visitor.message.unwrap_or_default(),
      origin:           visitor.origin.or_else(|| Some(String::from("n/a"))),
      timestamp_micros: visitor.timestamp_micros,
    };
    self.events.lock().expect("lock").push(captured);
  }

  fn enter(&self, _: &Id) {}

  fn exit(&self, _: &Id) {}
}

#[derive(Clone, Debug)]
struct CapturedEvent {
  level:            Level,
  target:           String,
  message:          String,
  origin:           Option<String>,
  timestamp_micros: Option<u64>,
}

impl Default for CapturedEvent {
  fn default() -> Self {
    Self {
      level:            Level::INFO,
      target:           String::new(),
      message:          String::new(),
      origin:           None,
      timestamp_micros: None,
    }
  }
}

#[derive(Default)]
struct EventVisitor {
  message:          Option<String>,
  origin:           Option<String>,
  timestamp_micros: Option<u64>,
}

impl Visit for EventVisitor {
  fn record_str(&mut self, field: &Field, value: &str) {
    match field.name() {
      | "message" => self.message = Some(value.to_owned()),
      | "origin" => self.origin = Some(value.to_owned()),
      | _ => {},
    }
  }

  fn record_u64(&mut self, field: &Field, value: u64) {
    if field.name() == "timestamp_micros" {
      self.timestamp_micros = Some(value);
    }
  }

  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    if field.name() == "message" && self.message.is_none() {
      self.message = Some(format_value(value));
    } else if field.name() == "origin" && self.origin.is_none() {
      self.origin = Some(format_value(value));
    }
  }
}

fn format_value(value: &dyn fmt::Debug) -> String {
  let rendered = format!("{value:?}");
  rendered.trim_matches('"').to_owned()
}
