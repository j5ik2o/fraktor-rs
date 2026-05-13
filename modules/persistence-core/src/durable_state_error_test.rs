use alloc::string::ToString;

use crate::durable_state_error::DurableStateError;

#[test]
fn durable_state_error_display_get_object_failed() {
  let error = DurableStateError::GetObjectFailed("missing".into());

  assert_eq!(error.to_string(), "get durable state object failed: missing");
}

#[test]
fn durable_state_error_display_upsert_object_failed() {
  let error = DurableStateError::UpsertObjectFailed("version conflict".into());

  assert_eq!(error.to_string(), "upsert durable state object failed: version conflict");
}

#[test]
fn durable_state_error_display_delete_object_failed() {
  let error = DurableStateError::DeleteObjectFailed("permission denied".into());

  assert_eq!(error.to_string(), "delete durable state object failed: permission denied");
}

#[test]
fn durable_state_error_display_changes_failed() {
  let error = DurableStateError::ChangesFailed("stream closed".into());

  assert_eq!(error.to_string(), "durable state changes failed: stream closed");
}

#[test]
fn durable_state_error_display_provider_already_registered() {
  let error = DurableStateError::provider_already_registered("in-memory");

  assert_eq!(error.to_string(), "durable state provider 'in-memory' already exists");
}

#[test]
fn durable_state_error_display_provider_not_found() {
  let error = DurableStateError::provider_not_found("missing-provider");

  assert_eq!(error.to_string(), "durable state provider 'missing-provider' not found");
}
