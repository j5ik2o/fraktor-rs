//! Failure detector configuration validation errors.

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
