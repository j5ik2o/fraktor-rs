//! Backoff configuration.

use core::time::Duration;

/// Configures restart backoff for the hidden persistence store actor.
#[derive(Clone, Debug, PartialEq)]
pub struct BackoffConfig {
  min_backoff:   Duration,
  max_backoff:   Duration,
  random_factor: f64,
}

impl BackoffConfig {
  /// Creates a backoff configuration.
  #[must_use]
  pub const fn new(min_backoff: Duration, max_backoff: Duration, random_factor: f64) -> Self {
    Self { min_backoff, max_backoff, random_factor }
  }

  /// Returns the minimum backoff duration.
  #[must_use]
  pub const fn min_backoff(&self) -> Duration {
    self.min_backoff
  }

  /// Returns the maximum backoff duration.
  #[must_use]
  pub const fn max_backoff(&self) -> Duration {
    self.max_backoff
  }

  /// Returns the random jitter factor.
  #[must_use]
  pub const fn random_factor(&self) -> f64 {
    self.random_factor
  }
}

impl Default for BackoffConfig {
  fn default() -> Self {
    Self { min_backoff: Duration::from_secs(1), max_backoff: Duration::from_secs(60), random_factor: 0.2 }
  }
}
