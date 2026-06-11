use super::ClusterSingletonSettingsError;

#[test]
fn display_empty_singleton_name_contains_cause() {
  let err = ClusterSingletonSettingsError::EmptySingletonName;
  let msg = alloc::format!("{err}");
  assert!(msg.contains("singleton name"), "Display should mention 'singleton name', got: {msg}");
}

#[test]
fn display_buffer_size_out_of_range_contains_cause_and_value() {
  let err = ClusterSingletonSettingsError::BufferSizeOutOfRange { value: 99999 };
  let msg = alloc::format!("{err}");
  assert!(msg.contains("buffer size"), "Display should mention 'buffer size', got: {msg}");
  assert!(msg.contains("99999"), "Display should include the offending value, got: {msg}");
}

#[test]
fn display_non_positive_hand_over_retry_interval_contains_cause() {
  let err = ClusterSingletonSettingsError::NonPositiveHandOverRetryInterval;
  let msg = alloc::format!("{err}");
  assert!(msg.contains("hand-over retry interval"), "Display should mention 'hand-over retry interval', got: {msg}");
}

#[test]
fn display_non_positive_identification_interval_contains_cause() {
  let err = ClusterSingletonSettingsError::NonPositiveIdentificationInterval;
  let msg = alloc::format!("{err}");
  assert!(msg.contains("identification interval"), "Display should mention 'identification interval', got: {msg}");
}

#[test]
fn display_empty_lease_implementation_contains_cause() {
  let err = ClusterSingletonSettingsError::EmptyLeaseImplementation;
  let msg = alloc::format!("{err}");
  assert!(msg.contains("lease implementation"), "Display should mention 'lease implementation', got: {msg}");
}

#[test]
fn display_non_positive_lease_retry_interval_contains_cause() {
  let err = ClusterSingletonSettingsError::NonPositiveLeaseRetryInterval;
  let msg = alloc::format!("{err}");
  assert!(msg.contains("lease retry interval"), "Display should mention 'lease retry interval', got: {msg}");
}
