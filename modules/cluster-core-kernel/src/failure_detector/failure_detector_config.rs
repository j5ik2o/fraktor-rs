//! Failure detector observation configuration.

use core::time::Duration;

#[cfg(test)]
#[path = "failure_detector_config_test.rs"]
mod tests;

/// Failure detector observation configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FailureDetectorConfig {
  phi_threshold:              f64,
  max_sample_size:            usize,
  min_standard_deviation:     Duration,
  acceptable_heartbeat_pause: Duration,
  first_heartbeat_estimate:   Duration,
}

impl FailureDetectorConfig {
  /// Creates a failure detector configuration with defaults.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      phi_threshold:              1.0,
      max_sample_size:            10,
      min_standard_deviation:     Duration::from_millis(1),
      acceptable_heartbeat_pause: Duration::from_millis(0),
      first_heartbeat_estimate:   Duration::from_millis(10),
    }
  }

  /// Sets the phi threshold.
  #[must_use]
  pub const fn with_phi_threshold(mut self, value: f64) -> Self {
    self.phi_threshold = value;
    self
  }

  /// Sets the maximum sample size.
  #[must_use]
  pub const fn with_max_sample_size(mut self, value: usize) -> Self {
    self.max_sample_size = value;
    self
  }

  /// Sets the minimum standard deviation.
  #[must_use]
  pub const fn with_min_standard_deviation(mut self, value: Duration) -> Self {
    self.min_standard_deviation = value;
    self
  }

  /// Sets the acceptable heartbeat pause.
  #[must_use]
  pub const fn with_acceptable_heartbeat_pause(mut self, value: Duration) -> Self {
    self.acceptable_heartbeat_pause = value;
    self
  }

  /// Sets the first heartbeat estimate.
  #[must_use]
  pub const fn with_first_heartbeat_estimate(mut self, value: Duration) -> Self {
    self.first_heartbeat_estimate = value;
    self
  }

  /// Returns the phi threshold.
  #[must_use]
  pub const fn phi_threshold(&self) -> f64 {
    self.phi_threshold
  }

  /// Returns the maximum sample size.
  #[must_use]
  pub const fn max_sample_size(&self) -> usize {
    self.max_sample_size
  }

  /// Returns the minimum standard deviation.
  #[must_use]
  pub const fn min_standard_deviation(&self) -> Duration {
    self.min_standard_deviation
  }

  /// Returns the acceptable heartbeat pause.
  #[must_use]
  pub const fn acceptable_heartbeat_pause(&self) -> Duration {
    self.acceptable_heartbeat_pause
  }

  /// Returns the first heartbeat estimate.
  #[must_use]
  pub const fn first_heartbeat_estimate(&self) -> Duration {
    self.first_heartbeat_estimate
  }
}

impl Default for FailureDetectorConfig {
  fn default() -> Self {
    Self::new()
  }
}
