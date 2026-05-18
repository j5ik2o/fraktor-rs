//! Event payload describing a log entry emitted by the runtime.

#[cfg(test)]
#[path = "log_event_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, string::String};
use core::time::Duration;

use super::LogLevel;
use crate::actor::Pid;

/// Structured representation of a runtime log event.
#[derive(Clone, Debug)]
pub struct LogEvent {
  level:             LogLevel,
  message:           String,
  timestamp:         Duration,
  origin:            Option<Pid>,
  logger_name:       Option<String>,
  marker_name:       Option<String>,
  marker_properties: BTreeMap<String, String>,
  mdc:               BTreeMap<String, String>,
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
    Self {
      level,
      message,
      timestamp,
      origin,
      logger_name,
      marker_name: None,
      marker_properties: BTreeMap::new(),
      mdc: BTreeMap::new(),
    }
  }

  /// Attaches structured marker metadata to the event.
  #[must_use]
  pub fn with_marker(mut self, marker_name: impl Into<String>, marker_properties: BTreeMap<String, String>) -> Self {
    self.marker_name = Some(marker_name.into());
    self.marker_properties = marker_properties;
    self
  }

  /// Attaches structured MDC metadata to the event.
  #[must_use]
  pub fn with_mdc(mut self, mdc: BTreeMap<String, String>) -> Self {
    self.mdc = mdc;
    self
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

  /// Returns the marker name, if any.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn marker_name(&self) -> Option<&str> {
    self.marker_name.as_deref()
  }

  /// Returns the structured marker properties.
  #[must_use]
  pub const fn marker_properties(&self) -> &BTreeMap<String, String> {
    &self.marker_properties
  }

  /// Returns the structured MDC entries.
  #[must_use]
  pub const fn mdc(&self) -> &BTreeMap<String, String> {
    &self.mdc
  }
}
