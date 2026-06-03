use alloc::{string::String, vec, vec::Vec};

use fraktor_actor_core_kernel_rs::serialization::{
  builtin::{STRING_ID, register_defaults},
  default_serialization_setup,
  serialization_registry::SerializationRegistry,
};
use fraktor_cluster_core_kernel_rs::pub_sub::{PubSubBatch, PubSubEnvelope};
use fraktor_utils_core_rs::sync::ArcShared;

use super::deserialize_batch;

fn make_registry() -> ArcShared<SerializationRegistry> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  register_defaults(&registry, |_name, _id| {}).expect("register default serializers");
  registry
}

fn string_bytes(value: &str) -> Vec<u8> {
  let mut bytes = Vec::with_capacity(4 + value.len());
  bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
  bytes.extend_from_slice(value.as_bytes());
  bytes
}

#[test]
fn deserialize_batch_accepts_builtin_serializer_id() {
  let registry = make_registry();
  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: STRING_ID.value(),
    type_name:     String::from("String"),
    bytes:         string_bytes("mediated"),
  }]);

  let messages = deserialize_batch(&registry, &batch).expect("builtin string");

  assert_eq!(messages.len(), 1);
  assert_eq!(messages[0].downcast_ref::<String>().map(String::as_str), Some("mediated"));
}
