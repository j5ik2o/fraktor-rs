//! Failure detector observation configuration.

use alloc::vec::Vec;
use core::time::Duration;

use super::FailureDetectorConfigError;

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

  /// Validates this failure detector configuration.
  ///
  /// # Errors
  ///
  /// Returns [`FailureDetectorConfigError`] when an observation parameter is
  /// outside the accepted configuration range.
  pub fn validate(&self) -> Result<(), FailureDetectorConfigError> {
    if !self.phi_threshold.is_finite() || self.phi_threshold <= 0.0 {
      return Err(FailureDetectorConfigError::InvalidPhiThreshold);
    }
    if self.max_sample_size == 0 {
      return Err(FailureDetectorConfigError::ZeroMaxSampleSize);
    }
    if self.min_standard_deviation == Duration::ZERO {
      return Err(FailureDetectorConfigError::ZeroMinStandardDeviation);
    }
    if self.first_heartbeat_estimate == Duration::ZERO {
      return Err(FailureDetectorConfigError::ZeroFirstHeartbeatEstimate);
    }

    Ok(())
  }

  /// Returns observation parameter names whose values differ from another configuration.
  #[must_use]
  pub fn difference_field_names(&self, other: &Self) -> Vec<&'static str> {
    let mut names = Vec::new();

    if self.phi_threshold.to_bits() != other.phi_threshold.to_bits() {
      names.push("phi_threshold");
    }
    if self.max_sample_size != other.max_sample_size {
      names.push("max_sample_size");
    }
    if self.min_standard_deviation != other.min_standard_deviation {
      names.push("min_standard_deviation");
    }
    if self.acceptable_heartbeat_pause != other.acceptable_heartbeat_pause {
      names.push("acceptable_heartbeat_pause");
    }
    if self.first_heartbeat_estimate != other.first_heartbeat_estimate {
      names.push("first_heartbeat_estimate");
    }

    names
  }
}

impl Default for FailureDetectorConfig {
  fn default() -> Self {
    Self::new()
  }
}
