use fraktor_actor_rs::core::kernel::{error::SendError, messaging::AnyMessage};

use crate::core::r#impl::StreamError;

// --- StreamDetached variant ---

#[test]
fn stream_detached_is_constructible() {
  // Given/When: constructing the StreamDetached variant
  let error = StreamError::StreamDetached;

  // Then: it matches the expected variant
  assert!(matches!(error, StreamError::StreamDetached));
}

#[test]
fn stream_detached_display_contains_detached_message() {
  // Given: a StreamDetached error
  let error = StreamError::StreamDetached;

  // When: formatting with Display
  let message = alloc::format!("{error}");

  // Then: the message describes the detached state
  assert!(message.contains("detached"), "expected 'detached' in message: {message}");
}

#[test]
fn stream_detached_is_distinct_from_never_materialized() {
  // Given: both error variants
  let detached = StreamError::StreamDetached;
  let never_mat = StreamError::NeverMaterialized;

  // Then: they are not equal
  assert_ne!(detached, never_mat);
}

#[test]
fn stream_detached_clone_preserves_variant() {
  // Given: a StreamDetached error
  let original = StreamError::StreamDetached;

  // When: cloning
  let cloned = original.clone();

  // Then: clone equals original
  assert_eq!(original, cloned);
}

#[test]
fn stream_error_from_send_error_preserves_send_context() {
  let error = StreamError::from_send_error(&SendError::closed(AnyMessage::new("payload")));

  assert!(error.is_source_type::<SendError>());
  assert!(matches!(error, StreamError::FailedWithContext { .. }));
  assert!(alloc::format!("{error}").contains("send failed"));
}

#[test]
fn stream_error_from_send_error_maps_backpressure_to_would_block() {
  let error = StreamError::from_send_error(&SendError::full(AnyMessage::new("payload")));

  assert_eq!(error, StreamError::WouldBlock);
}
