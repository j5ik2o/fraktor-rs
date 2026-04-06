use alloc::string::ToString;

use crate::core::durable_state_exception::DurableStateException;

#[test]
fn durable_state_exception_display_get_object_failed() {
  let error = DurableStateException::GetObjectFailed("missing".into());

  assert_eq!(error.to_string(), "get durable state object failed: missing");
}

#[test]
fn durable_state_exception_display_upsert_object_failed() {
  let error = DurableStateException::UpsertObjectFailed("version conflict".into());

  assert_eq!(error.to_string(), "upsert durable state object failed: version conflict");
}

#[test]
fn durable_state_exception_display_delete_object_failed() {
  let error = DurableStateException::DeleteObjectFailed("permission denied".into());

  assert_eq!(error.to_string(), "delete durable state object failed: permission denied");
}

#[test]
fn durable_state_exception_display_changes_failed() {
  let error = DurableStateException::ChangesFailed("stream closed".into());

  assert_eq!(error.to_string(), "durable state changes failed: stream closed");
}

#[test]
fn durable_state_exception_display_provider_already_registered() {
  let error = DurableStateException::provider_already_registered("in-memory");

  assert_eq!(error.to_string(), "durable state provider 'in-memory' already exists");
}

#[test]
fn durable_state_exception_display_provider_not_found() {
  let error = DurableStateException::provider_not_found("missing-provider");

  assert_eq!(error.to_string(), "durable state provider 'missing-provider' not found");
}
