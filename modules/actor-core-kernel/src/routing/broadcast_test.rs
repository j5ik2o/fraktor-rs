use super::*;
use crate::actor::messaging::AnyMessage;

#[test]
fn broadcast_wraps_message() {
  // Given: a message payload
  let inner = AnyMessage::new(42_u32);

  // When: wrapping it in a Broadcast
  let broadcast = Broadcast(inner);

  // Then: the inner message is accessible and preserves the payload
  assert_eq!(broadcast.0.payload().downcast_ref::<u32>(), Some(&42));
}

#[test]
fn broadcast_debug_is_non_empty() {
  // Given: a Broadcast wrapping a message
  let broadcast = Broadcast(AnyMessage::new("hello"));

  // When: formatting with Debug
  let debug_str = alloc::format!("{:?}", broadcast);

  // Then: the output is non-empty
  assert!(!debug_str.is_empty());
}
