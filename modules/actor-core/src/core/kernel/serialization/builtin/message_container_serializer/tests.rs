use alloc::{string::String, vec};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::MessageContainerSerializer;
use crate::core::kernel::{
  actor::{
    actor_selection::{ActorSelectionMessage, SelectionPathElement},
    messaging::AnyMessage,
  },
  serialization::{
    builtin::{MESSAGE_CONTAINER_ID, register_defaults},
    default_serialization_setup,
    error::SerializationError,
    serialization_registry::SerializationRegistry,
    serializer::Serializer,
  },
};

fn registry() -> ArcShared<SerializationRegistry> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  register_defaults(&registry, |_name, _id| {}).expect("builtins");
  registry
}

fn serializer(registry: &ArcShared<SerializationRegistry>) -> MessageContainerSerializer {
  MessageContainerSerializer::new(MESSAGE_CONTAINER_ID, registry.downgrade())
}

fn round_trip(message: ActorSelectionMessage) -> ActorSelectionMessage {
  let registry = registry();
  let serializer = serializer(&registry);
  let bytes = serializer.to_binary(&message).expect("selection message should encode");
  let decoded = serializer.from_binary(&bytes, None).expect("selection message should decode");
  *decoded.downcast::<ActorSelectionMessage>().expect("selection message")
}

#[test]
fn identifier_returns_configured_id() {
  let registry = registry();

  assert_eq!(serializer(&registry).identifier(), MESSAGE_CONTAINER_ID);
}

#[test]
fn include_manifest_is_false() {
  let registry = registry();

  assert!(!serializer(&registry).include_manifest());
}

#[test]
fn actor_selection_message_round_trips_with_nested_string_payload() {
  let message = ActorSelectionMessage::new(
    AnyMessage::new(String::from("payload")),
    vec![SelectionPathElement::ChildName(String::from("worker")), SelectionPathElement::Parent],
    true,
  );

  let decoded = round_trip(message);

  assert_eq!(decoded.elements(), &[
    SelectionPathElement::ChildName(String::from("worker")),
    SelectionPathElement::Parent
  ]);
  assert!(decoded.wildcard_fan_out());
  assert_eq!(decoded.message().downcast_ref::<String>(), Some(&String::from("payload")));
}

#[test]
fn non_selection_message_type_is_rejected() {
  let registry = registry();
  let serializer = serializer(&registry);
  let result = serializer.to_binary(&String::from("wrong"));

  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}

#[test]
fn invalid_element_tag_is_rejected() {
  let registry = registry();
  let serializer = serializer(&registry);
  let message = ActorSelectionMessage::new(AnyMessage::new(String::from("payload")), Vec::new(), false);
  let mut bytes = serializer.to_binary(&message).expect("selection message should encode");
  bytes.extend_from_slice(&[1, u8::MAX]);

  let result = serializer.from_binary(&bytes, None);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}
