use super::{ClusterExtensionConfigValidationError, ClusterShardingSettingsError};
use crate::failure_detector::{CrossDcFailureDetectorConfigError, FailureDetectorConfigError};

#[test]
fn display_failure_detector_variant_includes_nested_error() {
  let error = ClusterExtensionConfigValidationError::FailureDetector(FailureDetectorConfigError::InvalidPhiThreshold);
  assert!(error.to_string().contains("failure detector config"));
  assert!(error.to_string().contains("phi threshold"));
}

#[test]
fn display_sharding_settings_variant_includes_nested_error() {
  let error = ClusterExtensionConfigValidationError::ShardingSettings(ClusterShardingSettingsError::ZeroNumberOfShards);
  assert!(error.to_string().contains("sharding settings"));
}

#[test]
fn display_cross_dc_failure_detector_variant_includes_nested_error() {
  let error = ClusterExtensionConfigValidationError::CrossDcFailureDetector(
    CrossDcFailureDetectorConfigError::ZeroHeartbeatInterval,
  );
  assert!(error.to_string().contains("cross-DC failure detector config"));
}
