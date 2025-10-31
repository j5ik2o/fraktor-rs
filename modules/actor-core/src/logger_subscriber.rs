//! Event stream subscriber that forwards log events to a writer implementation.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  event_stream_event::EventStreamEvent, event_stream_subscriber::EventStreamSubscriber, log_level::LogLevel,
  logger_writer::LoggerWriter,
};

/// Subscriber filtering log events by level and delegating to a writer.
pub struct LoggerSubscriber {
  level:  LogLevel,
  writer: ArcShared<dyn LoggerWriter>,
}

impl LoggerSubscriber {
  /// Creates a new logger subscriber.
  #[must_use]
  pub fn new(level: LogLevel, writer: ArcShared<dyn LoggerWriter>) -> Self {
    Self { level, writer }
  }

  /// Returns the minimum severity handled by this subscriber.
  #[must_use]
  pub const fn level(&self) -> LogLevel {
    self.level
  }
}

impl EventStreamSubscriber for LoggerSubscriber {
  fn on_event(&self, event: &EventStreamEvent) {
    if let EventStreamEvent::Log(log) = event {
      if log.level() >= self.level {
        self.writer.write(log);
      }
    }
  }
}
