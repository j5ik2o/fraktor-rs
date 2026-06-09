use core::time::Duration;

use super::FailureDetectorConfigError;
use crate::failure_detector::FailureDetectorConfig;

#[test]
fn identifies_phi_threshold_validation_failure() {
  let config = FailureDetectorConfig::new().with_phi_threshold(0.0);

  assert_eq!(config.validate(), Err(FailureDetectorConfigError::InvalidPhiThreshold));
}

#[test]
fn identifies_max_sample_size_validation_failure() {
  let config = FailureDetectorConfig::new().with_max_sample_size(0);

  assert_eq!(config.validate(), Err(FailureDetectorConfigError::ZeroMaxSampleSize));
}

#[test]
fn identifies_min_standard_deviation_validation_failure() {
  let config = FailureDetectorConfig::new().with_min_standard_deviation(Duration::ZERO);

  assert_eq!(config.validate(), Err(FailureDetectorConfigError::ZeroMinStandardDeviation));
}

#[test]
fn identifies_first_heartbeat_estimate_validation_failure() {
  let config = FailureDetectorConfig::new().with_first_heartbeat_estimate(Duration::ZERO);

  assert_eq!(config.validate(), Err(FailureDetectorConfigError::ZeroFirstHeartbeatEstimate));
}
