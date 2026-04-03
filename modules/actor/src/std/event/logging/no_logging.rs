//! No-op classic logging facade.

#[cfg(test)]
mod tests;

use crate::core::kernel::event::logging::LogLevel;

/// No-op logger matching Pekko's `NoLogging` intent.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoLogging;

impl NoLogging {
  /// Ignores the provided severity and message.
  pub fn log(&self, _level: LogLevel, _message: impl Into<alloc::string::String>) {}

  /// Ignores a trace-level message.
  pub fn trace(&self, _message: impl Into<alloc::string::String>) {}

  /// Ignores a debug-level message.
  pub fn debug(&self, _message: impl Into<alloc::string::String>) {}

  /// Ignores an info-level message.
  pub fn info(&self, _message: impl Into<alloc::string::String>) {}

  /// Ignores a warn-level message.
  pub fn warn(&self, _message: impl Into<alloc::string::String>) {}

  /// Ignores an error-level message.
  pub fn error(&self, _message: impl Into<alloc::string::String>) {}
}
