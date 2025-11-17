//! `tracing`-backed event stream subscriber for standard environments.

extern crate std;

#[cfg(test)]
mod tests;

use alloc::string::ToString;
use core::time::Duration;

use fraktor_utils_rs::core::sync::ArcShared;
use tracing::{Level, event};

use crate::{
  core::logging::{LogEvent, LogLevel, LoggerSubscriber, LoggerWriter},
  std::event_stream::{EventStreamEvent, EventStreamSubscriber},
};

/// Event stream subscriber that forwards runtime log events to the `tracing` crate.
pub struct TracingLoggerSubscriber {
  inner: LoggerSubscriber,
}

impl TracingLoggerSubscriber {
  /// Default target name used in emitted events.
  pub const DEFAULT_TARGET: &'static str = "fraktor::event_stream::log";

  /// Creates a subscriber with the provided minimum log level.
  #[must_use]
  pub fn new(level: LogLevel) -> Self {
    let writer = ArcShared::new(TracingLoggerWriter);
    Self { inner: LoggerSubscriber::new(level, writer) }
  }

  /// Returns the minimum severity handled by this subscriber.
  #[must_use]
  pub const fn level(&self) -> LogLevel {
    self.inner.level()
  }
}

impl EventStreamSubscriber for TracingLoggerSubscriber {
  fn on_event(&self, event: &EventStreamEvent) {
    self.inner.on_event(event);
  }
}

struct TracingLoggerWriter;

impl LoggerWriter for TracingLoggerWriter {
  fn write(&self, event: &LogEvent) {
    let timestamp_micros = duration_to_micros(event.timestamp());
    let origin = event.origin().map(|pid| pid.to_string());
    let origin_str = origin.as_deref().unwrap_or("n/a");
    let message = event.message();

    match event.level() {
      | LogLevel::Trace => {
        event!(
          target: TracingLoggerSubscriber::DEFAULT_TARGET,
          Level::TRACE,
          timestamp_micros = timestamp_micros,
          origin = origin_str,
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
          "{}",
          message
        );
      },
    }
  }
}

fn duration_to_micros(duration: Duration) -> u64 {
  let micros = duration.as_micros();
  if micros > u64::MAX as u128 { u64::MAX } else { micros as u64 }
}
