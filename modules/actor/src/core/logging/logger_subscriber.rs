//! Logger subscriber that forwards log events to a writer sink.

use alloc::boxed::Box;
use core::marker::PhantomData;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  logging::{LogLevel, logger_writer::LoggerWriter},
};

#[cfg(test)]
mod tests;

/// Subscribes to log events and filters by severity.
pub struct LoggerSubscriberGeneric<TB: RuntimeToolbox + 'static> {
  level:   LogLevel,
  writer:  Box<dyn LoggerWriter>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> LoggerSubscriberGeneric<TB> {
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

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriber<TB> for LoggerSubscriberGeneric<TB> {
  fn on_event(&mut self, event: &EventStreamEvent<TB>) {
    if let EventStreamEvent::Log(log) = event
      && log.level() >= self.level
    {
      self.writer.write(log);
    }
  }
}

/// Type alias for backward compatibility.
pub type LoggerSubscriber = LoggerSubscriberGeneric<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>;
