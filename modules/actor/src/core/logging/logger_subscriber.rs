//! Logger subscriber that forwards log events to a writer sink.

use alloc::boxed::Box;
use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  logging::{LogLevel, logger_writer::LoggerWriter},
};

#[cfg(test)]
mod tests;

/// Subscribes to log events and filters by severity.
pub struct LoggerSubscriberGeneric<TB: RuntimeToolbox + 'static> {
  level:   LogLevel,
  writer:  ArcShared<ToolboxMutex<Box<dyn LoggerWriter>, TB>>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> LoggerSubscriberGeneric<TB> {
  /// Creates a new subscriber with the provided minimum log level.
  #[must_use]
  pub fn new(level: LogLevel, writer: Box<dyn LoggerWriter>) -> Self {
    let writer_mutex: ToolboxMutex<Box<dyn LoggerWriter>, TB> = <TB::MutexFamily as SyncMutexFamily>::create(writer);
    Self { level, writer: ArcShared::new(writer_mutex), _marker: PhantomData }
  }

  /// Returns the minimum severity handled by this subscriber.
  #[must_use]
  pub const fn level(&self) -> LogLevel {
    self.level
  }
}

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriber<TB> for LoggerSubscriberGeneric<TB> {
  fn on_event(&self, event: &EventStreamEvent<TB>) {
    if let EventStreamEvent::Log(log) = event
      && log.level() >= self.level
    {
      self.writer.lock().write(log);
    }
  }
}

/// Type alias for backward compatibility.
pub type LoggerSubscriber = LoggerSubscriberGeneric<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>;
