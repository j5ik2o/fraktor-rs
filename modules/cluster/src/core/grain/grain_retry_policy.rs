//! Retry policy for grain calls.

use core::time::Duration;

/// Retry policy for grain calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrainRetryPolicy {
  /// Disable retries.
  NoRetry,
  /// Retry with a fixed delay.
  Fixed {
    /// Maximum retry count.
    max_retries: u32,
    /// Delay between retries.
    delay:       Duration,
  },
  /// Retry with exponential backoff.
  Backoff {
    /// Maximum retry count.
    max_retries: u32,
    /// Base delay.
    base_delay:  Duration,
    /// Maximum delay cap.
    max_delay:   Duration,
  },
}

impl GrainRetryPolicy {
  /// Returns the maximum retry count for this policy.
  #[must_use]
  pub const fn max_retries(&self) -> u32 {
    match self {
      | GrainRetryPolicy::NoRetry => 0,
      | GrainRetryPolicy::Fixed { max_retries, .. } => *max_retries,
      | GrainRetryPolicy::Backoff { max_retries, .. } => *max_retries,
    }
  }

  /// Returns the retry delay for the given retry attempt (0-based).
  #[must_use]
  pub(crate) fn retry_delay(&self, attempt: u32) -> Duration {
    match self {
      | GrainRetryPolicy::NoRetry => Duration::from_secs(0),
      | GrainRetryPolicy::Fixed { delay, .. } => *delay,
      | GrainRetryPolicy::Backoff { base_delay, max_delay, .. } => {
        let mut delay = *base_delay;
        for _ in 0..attempt {
          if delay >= *max_delay {
            return *max_delay;
          }
          delay = delay.checked_mul(2).unwrap_or(*max_delay);
        }
        if delay > *max_delay { *max_delay } else { delay }
      },
    }
  }
}
