//! Event payload describing a log entry emitted by the runtime.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::time::Duration;

use super::LogLevel;
use crate::core::kernel::actor::Pid;

/// Structured representation of a runtime log event.
#[derive(Clone, Debug)]
pub struct LogEvent {
  level:       LogLevel,
  message:     String,
  timestamp:   Duration,
  origin:      Option<Pid>,
  logger_name: Option<String>,
}

impl LogEvent {
  /// Creates a new log event.
  ///
  /// `logger_name` corresponds to Pekko's `ActorContext.setLoggerName` and
  /// allows per-actor customisation of the tracing target.
  #[must_use]
  pub const fn new(
    level: LogLevel,
    message: String,
    timestamp: Duration,
    origin: Option<Pid>,
    logger_name: Option<String>,
  ) -> Self {
    Self { level, message, timestamp, origin, logger_name }
  }

  /// Returns the severity level.
  #[must_use]
  pub const fn level(&self) -> LogLevel {
    self.level
  }

  /// Returns the log message.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
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

  /// Returns the logger name override, if any.
  ///
  /// Corresponds to Pekko's `ActorContext.setLoggerName`.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn logger_name(&self) -> Option<&str> {
    self.logger_name.as_deref()
  }
}
