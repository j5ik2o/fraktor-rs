//! Logger subscriber that forwards log events to a writer sink.

use alloc::boxed::Box;
use core::marker::PhantomData;

use crate::core::event::{
  logging::{LogLevel, logger_writer::LoggerWriter},
  stream::{EventStreamEvent, EventStreamSubscriber},
};

#[cfg(test)]
mod tests;

/// Subscribes to log events and filters by severity.
pub struct LoggerSubscriber {
  level:   LogLevel,
  writer:  Box<dyn LoggerWriter>,
  _marker: PhantomData<()>,
}

impl LoggerSubscriber {
  /// Creates a new subscriber with the provided minimum log level.
  #[must_use]
  pub fn new(level: LogLevel, writer: Box<dyn LoggerWriter>) -> Self {
    Self { level, writer, _marker: PhantomData }
  }

  /// Returns the minimum severity handled by this subscriber.
  #[must_use]
  pub const fn level(&self) -> LogLevel {
    self.level
  }
}

impl EventStreamSubscriber for LoggerSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Log(log) = event
      && log.level() >= self.level
    {
      self.writer.write(log);
    }
  }
}
