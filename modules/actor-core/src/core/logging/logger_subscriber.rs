//! Logger subscriber that forwards log events to a writer sink.

use fraktor_utils_core_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  logging::{LogLevel, logger_writer::LoggerWriter},
};

#[cfg(test)]
mod tests;

/// Subscribes to log events and filters by severity.
pub struct LoggerSubscriber {
  level:  LogLevel,
  writer: ArcShared<dyn LoggerWriter>,
}

impl LoggerSubscriber {
  /// Creates a new subscriber with the provided minimum log level.
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

impl<TB: RuntimeToolbox> EventStreamSubscriber<TB> for LoggerSubscriber {
  fn on_event(&self, event: &EventStreamEvent<TB>) {
    if let EventStreamEvent::Log(log) = event
      && log.level() >= self.level
    {
      self.writer.write(log);
    }
  }
}
