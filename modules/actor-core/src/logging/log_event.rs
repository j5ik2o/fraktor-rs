//! Event payload describing a log entry emitted by the runtime.

use alloc::string::String;
use core::time::Duration;

use crate::{actor_prim::Pid, logging::LogLevel};

/// Structured representation of a runtime log event.
#[derive(Clone, Debug)]
pub struct LogEvent {
  level:     LogLevel,
  message:   String,
  timestamp: Duration,
  origin:    Option<Pid>,
}

impl LogEvent {
  /// Creates a new log event.
  #[must_use]
  pub const fn new(level: LogLevel, message: String, timestamp: Duration, origin: Option<Pid>) -> Self {
    Self { level, message, timestamp, origin }
  }

  /// Returns the severity level.
  #[must_use]
  pub const fn level(&self) -> LogLevel {
    self.level
  }

  /// Returns the log message.
  #[must_use]
  pub fn message(&self) -> &str {
    &self.message
  }

  /// Returns the originating actor pid, if any.
  #[must_use]
  pub const fn origin(&self) -> Option<Pid> {
    self.origin
  }

  /// Returns the timestamp associated with the event.
  #[must_use]
  pub const fn timestamp(&self) -> Duration {
    self.timestamp
  }
}
