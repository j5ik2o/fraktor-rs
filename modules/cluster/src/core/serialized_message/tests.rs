use crate::core::serialized_message::SerializedMessage;

#[test]
fn emptiness_is_detected() {
  let m = SerializedMessage::new(Vec::new(), 1);
  assert!(m.is_empty());
  let filled = SerializedMessage::new(vec![1], 1);
  assert!(!filled.is_empty());
}
