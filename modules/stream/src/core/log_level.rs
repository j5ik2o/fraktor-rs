//! Log level definitions for stream attribute configuration.

#[cfg(test)]
mod tests;

use core::any::Any;

use super::Attribute;

/// Log level for stream stage diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
  /// Logging disabled.
  Off,
  /// Error-level logging.
  Error,
  /// Warning-level logging.
  Warning,
  /// Info-level logging.
  Info,
  /// Debug-level logging.
  Debug,
}

impl Attribute for LogLevel {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> alloc::boxed::Box<dyn Attribute> {
    alloc::boxed::Box::new(*self)
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}
