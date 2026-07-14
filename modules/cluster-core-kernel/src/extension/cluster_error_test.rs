use super::{ClusterError, ClusterExtensionConfigError};
use crate::failure_detector::FailureDetectorConfigError;

#[test]
fn configuration_validation_failure_is_lifecycle_error() {
  let error = ClusterError::Configuration(ClusterExtensionConfigError::FailureDetector(
    FailureDetectorConfigError::InvalidPhiThreshold,
  ));

  assert_eq!(
    ClusterError::Configuration(ClusterExtensionConfigError::FailureDetector(
      FailureDetectorConfigError::InvalidPhiThreshold
    )),
    error
  );
}

#[test]
fn converts_failure_detector_config_error_to_cluster_error() {
  let error: ClusterError = FailureDetectorConfigError::InvalidPhiThreshold.into();

  assert_eq!(
    ClusterError::Configuration(ClusterExtensionConfigError::FailureDetector(
      FailureDetectorConfigError::InvalidPhiThreshold
    )),
    error
  );
}
