//! Configuration controlling the phi failure detector behaviour.

/// Configuration controlling the phi failure detector behaviour.
#[derive(Clone, Debug)]
pub struct PhiFailureDetectorConfig {
  threshold:           f64,
  max_sample_size:     usize,
  minimum_interval_ms: u64,
}

impl PhiFailureDetectorConfig {
  /// Creates a new configuration.
  #[must_use]
  pub const fn new(threshold: f64, max_sample_size: usize, minimum_interval_ms: u64) -> Self {
    Self { threshold, max_sample_size, minimum_interval_ms }
  }

  /// Returns the configured threshold.
  #[must_use]
  pub const fn threshold(&self) -> f64 {
    self.threshold
  }

  /// Returns the configured maximum sample size.
  #[must_use]
  pub const fn max_sample_size(&self) -> usize {
    self.max_sample_size
  }

  /// Returns the configured minimum heartbeat interval in milliseconds.
  #[must_use]
  pub const fn minimum_interval_ms(&self) -> u64 {
    self.minimum_interval_ms
  }
}

impl Default for PhiFailureDetectorConfig {
  fn default() -> Self {
    Self { threshold: 8.0, max_sample_size: 100, minimum_interval_ms: 200 }
  }
}
