//! Pre-publish logging filter contract.

#[cfg(test)]
mod tests;

use super::{LogEvent, LogLevel};

/// Predicate used to decide whether a log event should reach the event stream.
pub trait LoggingFilter: Send + Sync {
  /// Returns `true` when the provided event should be published.
  fn should_publish(&self, event: &LogEvent) -> bool;

  /// Returns `true` when the filter would accept an event of the given `level`.
  ///
  /// The default implementation accepts every level. Implementations that gate
  /// publication by severity should override this method so that callers can
  /// skip expensive argument evaluation when the level is disabled.
  fn is_level_enabled(&self, _level: LogLevel) -> bool {
    true
  }
}
