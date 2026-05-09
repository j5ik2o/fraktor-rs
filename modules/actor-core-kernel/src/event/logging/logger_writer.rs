//! Trait implemented by logging sinks consuming log events.

use crate::event::logging::LogEvent;

/// Interface for log event writers.
pub trait LoggerWriter: Send + Sync {
  /// Writes the provided event to the underlying sink.
  fn write(&mut self, event: &LogEvent);
}
