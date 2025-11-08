use alloc::string::ToString;

use super::SerializerRegistry;
use crate::{
  NoStdToolbox,
  serialization::{BincodeSerializer, error::SerializationError, serializer::SerializerHandle},
};

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct Message(u32);

fn decode(bytes: &[u8]) -> Result<Message, SerializationError> {
  bincode::serde::decode_from_slice(bytes, bincode::config::standard().with_fixed_int_encoding())
    .map(|(value, _)| value)
    .map_err(|error| SerializationError::DeserializationFailed(error.to_string()))
}

#[test]
fn registers_serializers() {
  let registry = SerializerRegistry::<NoStdToolbox>::new();
  let handle = SerializerHandle::new(BincodeSerializer::new());
  registry.register_serializer(handle.clone()).expect("first register");
  let err = registry.register_serializer(handle).expect_err("duplicate");
  assert!(matches!(err, SerializationError::DuplicateSerializerId(1)));
}

#[test]
fn binds_and_recovers_types() {
  let registry = SerializerRegistry::<NoStdToolbox>::new();
  let handle = SerializerHandle::new(BincodeSerializer::new());
  registry.register_serializer(handle.clone()).expect("register");
  registry.bind_type::<Message, _>(&handle, Some("Message".into()), decode).expect("bind");
  assert!(registry.has_binding_for::<Message>());
  let binding = registry.find_binding_by_manifest(handle.identifier(), "Message").expect("manifest");
  let sample = Message(9);
  let erased: &dyn erased_serde::Serialize = &sample;
  let bytes = handle.serialize_erased(erased).expect("serialize");
  let recovered: Message = binding.deserialize_as(bytes.as_ref()).expect("deserialize");
  assert_eq!(recovered, Message(9));
}
