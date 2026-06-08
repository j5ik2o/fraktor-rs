use super::FailureDetectorConfigError;

#[test]
fn identifies_phi_threshold_validation_failure() {
  let error = FailureDetectorConfigError::InvalidPhiThreshold;

  assert!(matches!(error, FailureDetectorConfigError::InvalidPhiThreshold));
}

#[test]
fn identifies_max_sample_size_validation_failure() {
  let error = FailureDetectorConfigError::ZeroMaxSampleSize;

  assert!(matches!(error, FailureDetectorConfigError::ZeroMaxSampleSize));
}

#[test]
fn identifies_min_standard_deviation_validation_failure() {
  let error = FailureDetectorConfigError::ZeroMinStandardDeviation;

  assert!(matches!(error, FailureDetectorConfigError::ZeroMinStandardDeviation));
}

#[test]
fn identifies_first_heartbeat_estimate_validation_failure() {
  let error = FailureDetectorConfigError::ZeroFirstHeartbeatEstimate;

  assert!(matches!(error, FailureDetectorConfigError::ZeroFirstHeartbeatEstimate));
}
