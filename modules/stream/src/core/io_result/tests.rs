use crate::core::{io_result::IOResult, stream_error::StreamError};

#[test]
fn successful_io_result() {
  let result = IOResult::successful(1024);
  assert_eq!(result.count(), 1024);
  assert!(result.was_successful());
  assert!(result.error().is_none());
}

#[test]
fn failed_io_result() {
  let result = IOResult::failed(512, StreamError::Failed);
  assert_eq!(result.count(), 512);
  assert!(!result.was_successful());
  assert_eq!(result.error(), Some(&StreamError::Failed));
}

#[test]
fn with_count_produces_new_instance() {
  let original = IOResult::successful(100);
  let updated = original.with_count(200);
  assert_eq!(updated.count(), 200);
  assert!(updated.was_successful());
}

#[test]
fn with_status_produces_new_instance() {
  let original = IOResult::successful(100);
  let updated = original.with_status(Err(StreamError::Failed));
  assert_eq!(updated.count(), 100);
  assert!(!updated.was_successful());
  assert_eq!(updated.error(), Some(&StreamError::Failed));
}

#[test]
fn zero_count_successful() {
  let result = IOResult::successful(0);
  assert_eq!(result.count(), 0);
  assert!(result.was_successful());
}

#[test]
fn failed_to_successful_via_with_status() {
  let result = IOResult::failed(42, StreamError::BufferOverflow);
  let recovered = result.with_status(Ok(()));
  assert!(recovered.was_successful());
  assert_eq!(recovered.count(), 42);
}
