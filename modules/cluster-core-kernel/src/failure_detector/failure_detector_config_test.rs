use core::time::Duration;

use super::FailureDetectorConfig;

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
