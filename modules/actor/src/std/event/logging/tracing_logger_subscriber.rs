//! `tracing`-backed event stream subscriber for standard environments.

extern crate std;

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::ToString};
use core::time::Duration;

use tracing::{Level, event, field};

use crate::{
  core::kernel::event::{
    logging::{LogEvent, LogLevel, LoggerSubscriber, LoggerWriter},
    stream::{EventStreamEvent, EventStreamSubscriber as CoreEventStreamSubscriber},
  },
  std::event::stream::EventStreamSubscriber,
};

/// Event stream subscriber that forwards runtime log events to the `tracing` crate.
pub struct TracingLoggerSubscriber {
  inner: LoggerSubscriber,
}

impl TracingLoggerSubscriber {
  /// Default target name used in emitted events.
  pub const DEFAULT_TARGET: &'static str = "fraktor::event::stream::log";

  /// Creates a subscriber with the provided minimum log level.
  #[must_use]
  pub fn new(level: LogLevel) -> Self {
    let writer: Box<dyn LoggerWriter> = Box::new(TracingLoggerWriter);
    Self { inner: LoggerSubscriber::new(level, writer) }
  }

  /// Returns the minimum severity handled by this subscriber.
  #[must_use]
  pub const fn level(&self) -> LogLevel {
    self.inner.level()
  }
}

impl EventStreamSubscriber for TracingLoggerSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    CoreEventStreamSubscriber::on_event(&mut self.inner, event);
  }
}

struct TracingLoggerWriter;

impl LoggerWriter for TracingLoggerWriter {
  fn write(&mut self, event: &LogEvent) {
    let timestamp_micros = duration_to_micros(event.timestamp());
    let origin = event.origin().map(|pid| pid.to_string());
    let origin_str = origin.as_deref().unwrap_or("n/a");
    let message = event.message();
    let logger_name = event.logger_name().unwrap_or("n/a");
    let marker_name = event.marker_name().unwrap_or("n/a");
    let marker_properties = field::debug(event.marker_properties());
    let mdc = field::debug(event.mdc());

    match event.level() {
      | LogLevel::Trace => {
        event!(
          target: TracingLoggerSubscriber::DEFAULT_TARGET,
          Level::TRACE,
          timestamp_micros = timestamp_micros,
          origin = origin_str,
          logger_name = logger_name,
          marker_name = marker_name,
          marker_properties = marker_properties,
          mdc = mdc,
          "{}",
          message
        );
      },
      | LogLevel::Debug => {
        event!(
          target: TracingLoggerSubscriber::DEFAULT_TARGET,
          Level::DEBUG,
          timestamp_micros = timestamp_micros,
          origin = origin_str,
          logger_name = logger_name,
          marker_name = marker_name,
          marker_properties = marker_properties,
          mdc = mdc,
          "{}",
          message
        );
      },
      | LogLevel::Info => {
        event!(
          target: TracingLoggerSubscriber::DEFAULT_TARGET,
          Level::INFO,
          timestamp_micros = timestamp_micros,
          origin = origin_str,
          logger_name = logger_name,
          marker_name = marker_name,
          marker_properties = marker_properties,
          mdc = mdc,
          "{}",
          message
        );
      },
      | LogLevel::Warn => {
        event!(
          target: TracingLoggerSubscriber::DEFAULT_TARGET,
          Level::WARN,
          timestamp_micros = timestamp_micros,
          origin = origin_str,
          logger_name = logger_name,
          marker_name = marker_name,
          marker_properties = marker_properties,
          mdc = mdc,
          "{}",
          message
        );
      },
      | LogLevel::Error => {
        event!(
          target: TracingLoggerSubscriber::DEFAULT_TARGET,
          Level::ERROR,
          timestamp_micros = timestamp_micros,
          origin = origin_str,
          logger_name = logger_name,
          marker_name = marker_name,
          marker_properties = marker_properties,
          mdc = mdc,
          "{}",
          message
        );
      },
    }
  }
}

const fn duration_to_micros(duration: Duration) -> u64 {
  let micros = duration.as_micros();
  if micros > u64::MAX as u128 { u64::MAX } else { micros as u64 }
}
