#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::any::Any;

use super::{Attribute, LogLevel};

/// Configures log levels for different stream lifecycle events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogLevels {
  /// Log level for element processing events.
  pub on_element: LogLevel,
  /// Log level for stream completion events.
  pub on_finish:  LogLevel,
  /// Log level for stream failure events.
  pub on_failure: LogLevel,
}

impl LogLevels {
  /// Creates a new log levels configuration.
  #[must_use]
  pub const fn new(on_element: LogLevel, on_finish: LogLevel, on_failure: LogLevel) -> Self {
    Self { on_element, on_finish, on_failure }
  }
}

impl Attribute for LogLevels {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> Box<dyn Attribute> {
    Box::new(*self)
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}
