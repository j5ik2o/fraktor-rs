//! Logging options for std typed behavior helpers.

#[cfg(test)]
mod tests;

use alloc::string::String;

/// Logging options used by `Behaviors::log_messages_with_opts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogOptions {
  enabled:     bool,
  level:       tracing::Level,
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
  pub const fn with_level(mut self, level: tracing::Level) -> Self {
    self.level = level;
    self
  }

  /// Replaces the configured logger target name.
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
  pub const fn level(&self) -> tracing::Level {
    self.level
  }

  /// Returns the configured logger target name.
  #[must_use]
  pub fn logger_name(&self) -> Option<&str> {
    self.logger_name.as_deref()
  }
}

impl Default for LogOptions {
  fn default() -> Self {
    Self { enabled: true, level: tracing::Level::DEBUG, logger_name: None }
  }
}
