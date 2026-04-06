use crate::core::typed::delivery::StoreMessageSentAck;

#[test]
fn new_creates_ack_with_stored_seq_nr() {
  // Given: a sequence number
  let ack = StoreMessageSentAck::new(42);

  // Then: the stored_seq_nr should match
  assert_eq!(ack.stored_seq_nr(), 42);
}

#[test]
fn ack_with_zero_seq_nr() {
  // Given: zero sequence number (edge case - initial state)
  let ack = StoreMessageSentAck::new(0);

  // Then
  assert_eq!(ack.stored_seq_nr(), 0);
}

#[test]
fn ack_with_max_seq_nr() {
  // Given: maximum sequence number
  let ack = StoreMessageSentAck::new(u64::MAX);

  // Then
  assert_eq!(ack.stored_seq_nr(), u64::MAX);
}

#[test]
fn ack_clone_preserves_value() {
  // Given
  let original = StoreMessageSentAck::new(100);

  // When
  let cloned = original.clone();

  // Then
  assert_eq!(cloned.stored_seq_nr(), 100);
}

#[test]
fn ack_debug_is_non_empty() {
  // Given
  let ack = StoreMessageSentAck::new(1);

  // When
  let debug_str = alloc::format!("{:?}", ack);

  // Then
  assert!(!debug_str.is_empty());
}

#[test]
fn ack_partial_eq_compares_by_value() {
  // Given
  let a = StoreMessageSentAck::new(10);
  let b = StoreMessageSentAck::new(10);
  let c = StoreMessageSentAck::new(20);

  // Then
  assert_eq!(a, b);
  assert_ne!(a, c);
}
