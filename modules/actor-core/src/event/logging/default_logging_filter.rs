//! Default log-level based logging filter.

#[cfg(test)]
mod tests;

use super::{LogEvent, LogLevel, LoggingFilter};

/// Default pre-publish filter that accepts events at or above a minimum level.
pub struct DefaultLoggingFilter {
  minimum_level: LogLevel,
}

impl DefaultLoggingFilter {
  /// Creates a filter using the provided minimum severity.
  #[must_use]
  pub const fn new(minimum_level: LogLevel) -> Self {
    Self { minimum_level }
  }
}

impl Default for DefaultLoggingFilter {
  fn default() -> Self {
    Self::new(LogLevel::Trace)
  }
}

impl LoggingFilter for DefaultLoggingFilter {
  fn should_publish(&self, event: &LogEvent) -> bool {
    event.level() >= self.minimum_level
  }

  fn is_level_enabled(&self, level: LogLevel) -> bool {
    level >= self.minimum_level
  }
}
