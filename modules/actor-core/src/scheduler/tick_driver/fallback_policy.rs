//! Fallback policy for driver failures.

use core::time::Duration;

/// Policy for handling driver failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackPolicy {
  /// Retry with exponential backoff.
  Retry {
    /// Maximum number of retry attempts.
    attempts: u8,
    /// Initial backoff duration.
    backoff:  Duration,
  },
  /// Fail immediately without retry.
  FailFast,
}

impl Default for FallbackPolicy {
  fn default() -> Self {
    Self::Retry { attempts: 3, backoff: Duration::from_millis(50) }
  }
}
