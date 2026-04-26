use alloc::{string::String, vec};

use super::ActorSelectionMessage;
use crate::core::kernel::actor::{actor_selection::SelectionPathElement, messaging::AnyMessage};

#[test]
fn new_keeps_payload_elements_and_fanout_flag() {
  let elements = vec![SelectionPathElement::ChildName(String::from("worker")), SelectionPathElement::Parent];
  let message = ActorSelectionMessage::new(AnyMessage::new(String::from("payload")), elements.clone(), true);

  assert_eq!(message.elements(), elements.as_slice());
  assert!(message.wildcard_fan_out());
  assert_eq!(message.message().downcast_ref::<String>(), Some(&String::from("payload")));
}
