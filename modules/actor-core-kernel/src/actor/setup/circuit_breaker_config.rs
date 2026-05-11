//! Circuit-breaker configuration carried by actor-system bootstrap.

#[cfg(test)]
#[path = "circuit_breaker_config_test.rs"]
mod tests;

use core::time::Duration;

/// Pekko-compatible circuit-breaker configuration resolved by actor-system name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CircuitBreakerConfig {
  max_failures:  u32,
  reset_timeout: Duration,
}

impl CircuitBreakerConfig {
  /// Creates circuit-breaker configuration with the provided thresholds.
  ///
  /// # Panics
  ///
  /// Panics when `max_failures` is zero.
  #[must_use]
  pub fn new(max_failures: u32, reset_timeout: Duration) -> Self {
    assert!(max_failures > 0, "max_failures must be greater than zero");
    Self { max_failures, reset_timeout }
  }

  /// Returns a copy with a different maximum failure threshold.
  ///
  /// # Panics
  ///
  /// Panics when `max_failures` is zero.
  #[must_use]
  pub fn with_max_failures(self, max_failures: u32) -> Self {
    Self::new(max_failures, self.reset_timeout)
  }

  /// Returns a copy with a different reset timeout.
  #[must_use]
  pub const fn with_reset_timeout(self, reset_timeout: Duration) -> Self {
    Self { max_failures: self.max_failures, reset_timeout }
  }

  /// Returns the configured maximum failure threshold.
  #[must_use]
  pub const fn max_failures(&self) -> u32 {
    self.max_failures
  }

  /// Returns the configured reset timeout.
  #[must_use]
  pub const fn reset_timeout(&self) -> Duration {
    self.reset_timeout
  }
}

impl Default for CircuitBreakerConfig {
  fn default() -> Self {
    Self::new(5, Duration::from_secs(30))
  }
}
