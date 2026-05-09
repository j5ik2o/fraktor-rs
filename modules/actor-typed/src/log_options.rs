//! Logging options for typed behavior helpers.

#[cfg(test)]
mod tests;

use alloc::string::String;

use fraktor_actor_core_kernel_rs::event::logging::LogLevel;

/// Logging options used by `Behaviors::log_messages_with_opts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogOptions {
  enabled:     bool,
  level:       LogLevel,
  logger_name: Option<String>,
}

impl LogOptions {
  /// Creates a new log option set with default values.
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Enables or disables logging.
  #[must_use]
  pub const fn with_enabled(mut self, enabled: bool) -> Self {
    self.enabled = enabled;
    self
  }

  /// Replaces the configured log level.
  #[must_use]
  pub const fn with_level(mut self, level: LogLevel) -> Self {
    self.level = level;
    self
  }

  /// Replaces the configured logger name field emitted by tracing logs.
  #[must_use]
  pub fn with_logger_name(mut self, logger_name: impl Into<String>) -> Self {
    self.logger_name = Some(logger_name.into());
    self
  }

  /// Returns true when message logging is enabled.
  #[must_use]
  pub const fn enabled(&self) -> bool {
    self.enabled
  }

  /// Returns the configured log level.
  #[must_use]
  pub const fn level(&self) -> LogLevel {
    self.level
  }

  /// Returns the configured logger name field.
  #[must_use]
  pub fn logger_name(&self) -> Option<&str> {
    self.logger_name.as_deref()
  }
}

impl Default for LogOptions {
  fn default() -> Self {
    Self { enabled: true, level: LogLevel::Debug, logger_name: None }
  }
}
