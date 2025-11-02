//! Trait implemented by logging sinks consuming log events.

use crate::LogEvent;

/// Interface for log event writers.
pub trait LoggerWriter: Send + Sync {
  /// Writes the provided event to the underlying sink.
  fn write(&self, event: &LogEvent);
}
