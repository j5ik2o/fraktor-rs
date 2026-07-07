use super::CrossDcFailureDetectorConfigError;

#[test]
fn display_zero_heartbeat_interval_contains_cause() {
  let error = CrossDcFailureDetectorConfigError::ZeroHeartbeatInterval;
  assert!(error.to_string().contains("heartbeat interval"));
}

#[test]
fn display_zero_expected_response_after_contains_cause() {
  let error = CrossDcFailureDetectorConfigError::ZeroExpectedResponseAfter;
  assert!(error.to_string().contains("expected response after"));
}
