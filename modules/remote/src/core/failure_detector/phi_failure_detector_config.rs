//! Configuration for phi accrual failure detector.

use core::time::Duration;

/// Configuration options for
/// [`PhiFailureDetector`](super::phi_failure_detector::PhiFailureDetector).
#[derive(Clone, Debug)]
pub struct PhiFailureDetectorConfig {
  sample_size:  usize,
  threshold:    f64,
  min_interval: Duration,
}

impl PhiFailureDetectorConfig {
  /// Creates a configuration with sane defaults.
  #[must_use]
  pub const fn new(sample_size: usize, threshold: f64, min_interval: Duration) -> Self {
    Self { sample_size, threshold, min_interval }
  }

  /// Returns default configuration.
  #[must_use]
  pub const fn default() -> Self {
    Self::new(100, 8.0, Duration::from_millis(10))
  }

  /// Overrides the maximum number of samples kept per authority.
  #[must_use]
  pub const fn with_sample_size(mut self, sample_size: usize) -> Self {
    self.sample_size = sample_size;
    self
  }

  /// Overrides the phi threshold used to trigger suspect events.
  #[must_use]
  pub const fn with_threshold(mut self, threshold: f64) -> Self {
    self.threshold = threshold;
    self
  }

  /// Overrides the minimum heartbeat interval used to filter jitter.
  #[must_use]
  pub const fn with_min_interval(mut self, min_interval: Duration) -> Self {
    self.min_interval = min_interval;
    self
  }

  /// Returns configured sample size.
  #[must_use]
  pub const fn sample_size(&self) -> usize {
    self.sample_size
  }

  /// Returns configured phi threshold.
  #[must_use]
  pub const fn threshold(&self) -> f64 {
    self.threshold
  }

  /// Returns configured minimum heartbeat interval.
  #[must_use]
  pub const fn min_interval(&self) -> Duration {
    self.min_interval
  }
}

impl Default for PhiFailureDetectorConfig {
  fn default() -> Self {
    Self::new(100, 8.0, Duration::from_millis(10))
  }
}
