use core::time::Duration;

use super::{CrossDcFailureDetectorConfig, CrossDcFailureDetectorConfigError};

#[test]
fn defaults_match_pekko_compatible_values() {
  let config = CrossDcFailureDetectorConfig::new();
  assert_eq!(config.heartbeat_interval(), Duration::from_secs(3));
  assert_eq!(config.expected_response_after(), Duration::from_millis(600));
}

#[test]
fn builder_preserves_custom_values() {
  let config = CrossDcFailureDetectorConfig::new()
    .with_heartbeat_interval(Duration::from_secs(5))
    .with_expected_response_after(Duration::from_secs(1));

  assert_eq!(config.heartbeat_interval(), Duration::from_secs(5));
  assert_eq!(config.expected_response_after(), Duration::from_secs(1));
}

#[test]
fn validate_rejects_zero_heartbeat_interval() {
  let config = CrossDcFailureDetectorConfig::new().with_heartbeat_interval(Duration::ZERO);
  assert_eq!(config.validate(), Err(CrossDcFailureDetectorConfigError::ZeroHeartbeatInterval));
}

#[test]
fn validate_rejects_zero_expected_response_after() {
  let config = CrossDcFailureDetectorConfig::new().with_expected_response_after(Duration::ZERO);
  assert_eq!(config.validate(), Err(CrossDcFailureDetectorConfigError::ZeroExpectedResponseAfter));
}

#[test]
fn difference_field_names_reports_changed_fields() {
  let local = CrossDcFailureDetectorConfig::new();
  let joining = CrossDcFailureDetectorConfig::new().with_heartbeat_interval(Duration::from_secs(7));

  assert_eq!(local.difference_field_names(&joining), alloc::vec!["heartbeat_interval"]);
}

#[test]
fn validate_accepts_default_config() {
  assert_eq!(CrossDcFailureDetectorConfig::new().validate(), Ok(()));
}
