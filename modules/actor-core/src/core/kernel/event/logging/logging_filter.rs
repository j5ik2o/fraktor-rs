//! Pre-publish logging filter contract.

#[cfg(test)]
mod tests;

use super::LogEvent;

/// Predicate used to decide whether a log event should reach the event stream.
pub trait LoggingFilter: Send + Sync {
  /// Returns `true` when the provided event should be published.
  fn should_publish(&self, event: &LogEvent) -> bool;
}
