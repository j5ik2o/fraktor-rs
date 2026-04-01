use alloc::string::String;

use crate::core::typed::delivery::{MessageSent, NO_QUALIFIER};

#[test]
fn new_creates_message_sent_with_all_fields() {
  // Given: all fields for a MessageSent
  let sent = MessageSent::new(1, "hello", false, String::from("qualifier-A"), 1000);

  // Then: all accessors return the provided values
  assert_eq!(sent.seq_nr(), 1);
  assert_eq!(*sent.message(), "hello");
  assert!(!sent.ack());
  assert_eq!(sent.confirmation_qualifier(), "qualifier-A");
  assert_eq!(sent.timestamp_millis(), 1000);
}

#[test]
fn new_with_no_qualifier() {
  // Given: a MessageSent with NO_QUALIFIER
  let sent = MessageSent::new(1, 42_u32, true, NO_QUALIFIER.clone(), 0);

  // Then: confirmation qualifier should be empty
  assert!(sent.confirmation_qualifier().is_empty());
  assert!(sent.ack());
}

#[test]
fn message_sent_preserves_generic_type() {
  // Given: a MessageSent with a complex message type
  let sent = MessageSent::new(5, alloc::vec![1_u32, 2, 3], false, NO_QUALIFIER.clone(), 500);

  // Then: the message should be accessible with the correct type
  assert_eq!(sent.message().len(), 3);
  assert_eq!(sent.message()[0], 1);
}

#[test]
fn with_confirmation_qualifier_returns_new_instance() {
  // Given: a MessageSent
  let sent = MessageSent::new(1, "msg", false, NO_QUALIFIER.clone(), 100);

  // When: changing the confirmation qualifier
  let updated = sent.with_confirmation_qualifier(String::from("topic-B"));

  // Then: the qualifier is updated but other fields are preserved
  assert_eq!(updated.confirmation_qualifier(), "topic-B");
  assert_eq!(updated.seq_nr(), 1);
  assert_eq!(*updated.message(), "msg");
  assert!(!updated.ack());
  assert_eq!(updated.timestamp_millis(), 100);
}

#[test]
fn with_timestamp_millis_returns_new_instance() {
  // Given: a MessageSent
  let sent = MessageSent::new(2, "msg", true, String::from("q"), 100);

  // When: changing the timestamp
  let updated = sent.with_timestamp_millis(999);

  // Then: the timestamp is updated but other fields are preserved
  assert_eq!(updated.timestamp_millis(), 999);
  assert_eq!(updated.seq_nr(), 2);
  assert!(updated.ack());
}

#[test]
fn clone_preserves_all_fields() {
  // Given
  let sent = MessageSent::new(10, 42_u32, true, String::from("clone-test"), 5000);

  // When
  let cloned = sent.clone();

  // Then
  assert_eq!(cloned.seq_nr(), 10);
  assert_eq!(*cloned.message(), 42);
  assert!(cloned.ack());
  assert_eq!(cloned.confirmation_qualifier(), "clone-test");
  assert_eq!(cloned.timestamp_millis(), 5000);
}

#[test]
fn debug_format_is_non_empty() {
  // Given
  let sent = MessageSent::new(1, 0_u32, false, NO_QUALIFIER.clone(), 0);

  // When
  let debug_str = alloc::format!("{:?}", sent);

  // Then
  assert!(!debug_str.is_empty());
}

#[test]
fn partial_eq_compares_all_fields() {
  // Given
  let a = MessageSent::new(1, 42_u32, false, String::from("q"), 100);
  let b = MessageSent::new(1, 42_u32, false, String::from("q"), 100);
  let c = MessageSent::new(2, 42_u32, false, String::from("q"), 100);

  // Then
  assert_eq!(a, b);
  assert_ne!(a, c);
}
