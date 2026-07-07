//! Multi-DC failure detector settings namespace.

use alloc::vec::Vec;
use core::time::Duration;

use super::CrossDcFailureDetectorConfigError;

#[cfg(test)]
#[path = "cross_dc_failure_detector_config_test.rs"]
mod tests;

/// Cross-DC failure detector observation configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossDcFailureDetectorConfig {
  heartbeat_interval:      Duration,
  expected_response_after: Duration,
}

impl CrossDcFailureDetectorConfig {
  /// Creates a cross-DC failure detector configuration with defaults.
  #[must_use]
  pub const fn new() -> Self {
    Self { heartbeat_interval: Duration::from_secs(3), expected_response_after: Duration::from_millis(600) }
  }

  /// Sets the cross-DC heartbeat interval.
  #[must_use]
  pub const fn with_heartbeat_interval(mut self, interval: Duration) -> Self {
    self.heartbeat_interval = interval;
    self
  }

  /// Sets the expected response after interval.
  #[must_use]
  pub const fn with_expected_response_after(mut self, interval: Duration) -> Self {
    self.expected_response_after = interval;
    self
  }

  /// Returns the cross-DC heartbeat interval.
  #[must_use]
  pub const fn heartbeat_interval(&self) -> Duration {
    self.heartbeat_interval
  }

  /// Returns the expected response after interval.
  #[must_use]
  pub const fn expected_response_after(&self) -> Duration {
    self.expected_response_after
  }

  /// Validates this cross-DC failure detector configuration.
  ///
  /// # Errors
  ///
  /// Returns [`CrossDcFailureDetectorConfigError`] when an observation interval is zero.
  pub fn validate(&self) -> Result<(), CrossDcFailureDetectorConfigError> {
    if self.heartbeat_interval == Duration::ZERO {
      return Err(CrossDcFailureDetectorConfigError::ZeroHeartbeatInterval);
    }
    if self.expected_response_after == Duration::ZERO {
      return Err(CrossDcFailureDetectorConfigError::ZeroExpectedResponseAfter);
    }

    Ok(())
  }

  /// Returns observation parameter names whose values differ from another configuration.
  #[must_use]
  pub fn difference_field_names(&self, other: &Self) -> Vec<&'static str> {
    let mut names = Vec::new();

    if self.heartbeat_interval != other.heartbeat_interval {
      names.push("heartbeat_interval");
    }
    if self.expected_response_after != other.expected_response_after {
      names.push("expected_response_after");
    }

    names
  }
}

impl Default for CrossDcFailureDetectorConfig {
  fn default() -> Self {
    Self::new()
  }
}
