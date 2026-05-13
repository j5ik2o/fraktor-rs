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
  ///
  /// # Panics
  ///
  /// Panics if `min_backoff > max_backoff` or `random_factor` is outside `[0.0, 1.0]`.
  #[must_use]
  pub fn new(min_backoff: Duration, max_backoff: Duration, random_factor: f64) -> Self {
    assert!(min_backoff <= max_backoff, "min_backoff must not exceed max_backoff");
    assert!((0.0..=1.0).contains(&random_factor) && !random_factor.is_nan(), "random_factor must be in [0.0, 1.0]");
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
