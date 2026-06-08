use core::time::Duration;

use super::FailureDetectorConfig;
use crate::failure_detector::FailureDetectorConfigError;

#[test]
fn default_config_keeps_existing_observation_parameters() {
  let config = FailureDetectorConfig::new();

  assert_eq!(config.phi_threshold(), 1.0);
  assert_eq!(config.max_sample_size(), 10);
  assert_eq!(config.min_standard_deviation(), Duration::from_millis(1));
  assert_eq!(config.acceptable_heartbeat_pause(), Duration::from_millis(0));
  assert_eq!(config.first_heartbeat_estimate(), Duration::from_millis(10));
}

#[test]
fn custom_config_keeps_builder_style_observation_parameters() {
  let config = FailureDetectorConfig::new()
    .with_phi_threshold(8.5)
    .with_max_sample_size(42)
    .with_min_standard_deviation(Duration::from_millis(3))
    .with_acceptable_heartbeat_pause(Duration::from_millis(250))
    .with_first_heartbeat_estimate(Duration::from_millis(30));

  assert_eq!(config.phi_threshold(), 8.5);
  assert_eq!(config.max_sample_size(), 42);
  assert_eq!(config.min_standard_deviation(), Duration::from_millis(3));
  assert_eq!(config.acceptable_heartbeat_pause(), Duration::from_millis(250));
  assert_eq!(config.first_heartbeat_estimate(), Duration::from_millis(30));
}

#[test]
fn default_trait_uses_existing_observation_parameters() {
  let config = FailureDetectorConfig::default();

  assert_eq!(config.phi_threshold(), 1.0);
  assert_eq!(config.max_sample_size(), 10);
  assert_eq!(config.min_standard_deviation(), Duration::from_millis(1));
  assert_eq!(config.acceptable_heartbeat_pause(), Duration::from_millis(0));
  assert_eq!(config.first_heartbeat_estimate(), Duration::from_millis(10));
}

#[test]
fn validate_rejects_invalid_observation_parameters_but_allows_zero_acceptable_pause() {
  assert_eq!(FailureDetectorConfig::new().validate(), Ok(()));
  assert_eq!(
    FailureDetectorConfig::new().with_phi_threshold(0.0).validate(),
    Err(FailureDetectorConfigError::InvalidPhiThreshold)
  );
  assert_eq!(
    FailureDetectorConfig::new().with_phi_threshold(-1.0).validate(),
    Err(FailureDetectorConfigError::InvalidPhiThreshold)
  );
  assert_eq!(
    FailureDetectorConfig::new().with_phi_threshold(f64::NAN).validate(),
    Err(FailureDetectorConfigError::InvalidPhiThreshold)
  );
  assert_eq!(
    FailureDetectorConfig::new().with_phi_threshold(f64::INFINITY).validate(),
    Err(FailureDetectorConfigError::InvalidPhiThreshold)
  );
  assert_eq!(
    FailureDetectorConfig::new().with_max_sample_size(0).validate(),
    Err(FailureDetectorConfigError::ZeroMaxSampleSize)
  );
  assert_eq!(
    FailureDetectorConfig::new().with_min_standard_deviation(Duration::ZERO).validate(),
    Err(FailureDetectorConfigError::ZeroMinStandardDeviation)
  );
  assert_eq!(
    FailureDetectorConfig::new().with_first_heartbeat_estimate(Duration::ZERO).validate(),
    Err(FailureDetectorConfigError::ZeroFirstHeartbeatEstimate)
  );
  assert_eq!(FailureDetectorConfig::new().with_acceptable_heartbeat_pause(Duration::ZERO).validate(), Ok(()));
}

#[test]
fn difference_field_names_returns_only_changed_observation_parameter_names() {
  let base = FailureDetectorConfig::new();
  let changed = FailureDetectorConfig::new()
    .with_phi_threshold(8.5)
    .with_max_sample_size(42)
    .with_min_standard_deviation(Duration::from_millis(3))
    .with_acceptable_heartbeat_pause(Duration::from_millis(250))
    .with_first_heartbeat_estimate(Duration::from_millis(30));
  let max_sample_size_changed = FailureDetectorConfig::new().with_max_sample_size(42);

  assert!(base.difference_field_names(&base).is_empty());
  assert_eq!(max_sample_size_changed.difference_field_names(&base).as_slice(), ["max_sample_size"]);
  assert_eq!(changed.difference_field_names(&base).as_slice(), [
    "phi_threshold",
    "max_sample_size",
    "min_standard_deviation",
    "acceptable_heartbeat_pause",
    "first_heartbeat_estimate",
  ]);
}
