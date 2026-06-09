//! Failure detector configuration validation errors.

use core::{
  error::Error,
  fmt::{self, Formatter, Result as FmtResult},
};

#[cfg(test)]
#[path = "failure_detector_config_error_test.rs"]
mod tests;

/// Failure detector configuration validation errors.
///
/// These errors describe invalid configuration values and are not join
/// compatibility mismatch reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureDetectorConfigError {
  /// Phi threshold is not a positive finite value.
  InvalidPhiThreshold,
  /// Max sample size is zero.
  ZeroMaxSampleSize,
  /// Min standard deviation is zero.
  ZeroMinStandardDeviation,
  /// First heartbeat estimate is zero.
  ZeroFirstHeartbeatEstimate,
}

impl fmt::Display for FailureDetectorConfigError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::InvalidPhiThreshold => f.write_str("phi threshold must be a positive finite value"),
      | Self::ZeroMaxSampleSize => f.write_str("max sample size must be greater than zero"),
      | Self::ZeroMinStandardDeviation => f.write_str("min standard deviation must be greater than zero"),
      | Self::ZeroFirstHeartbeatEstimate => f.write_str("first heartbeat estimate must be greater than zero"),
    }
  }
}

impl Error for FailureDetectorConfigError {}
