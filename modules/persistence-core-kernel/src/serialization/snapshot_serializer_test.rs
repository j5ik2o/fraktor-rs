use alloc::{borrow::Cow, boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationError, SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId,
  SerializerWithStringManifest, serialization_registry::SerializationRegistry,
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::serialization::{
  SNAPSHOT_SERIALIZER_ID, SnapshotPayload, SnapshotSerializer, register_persistence_serializers,
};

const I32_MANIFEST: &str = "test.I32";

struct ManifestI32Serializer {
  id: SerializerId,
}

impl ManifestI32Serializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for ManifestI32Serializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let value = message.downcast_ref::<i32>().ok_or(SerializationError::InvalidFormat)?;
    Ok(value.to_le_bytes().to_vec())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    self.from_binary_with_manifest(bytes, I32_MANIFEST)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for ManifestI32Serializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed(I32_MANIFEST)
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    if manifest != I32_MANIFEST || bytes.len() != core::mem::size_of::<i32>() {
      return Err(SerializationError::InvalidFormat);
    }
    let mut array = [0_u8; core::mem::size_of::<i32>()];
    array.copy_from_slice(bytes);
    Ok(Box::new(i32::from_le_bytes(array)))
  }
}

fn registry() -> ArcShared<SerializationRegistry> {
  let id = SerializerId::try_from(110).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(ManifestI32Serializer::new(id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("i32", id, serializer)
    .expect("register")
    .set_fallback("i32")
    .expect("fallback")
    .bind::<i32>("i32")
    .expect("bind")
    .build()
    .expect("setup");
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  register_persistence_serializers(&registry).expect("persistence serializers");
  registry
}

#[test]
fn snapshot_payload_round_trip_restores_data() {
  let registry = registry();
  let serializer = SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade());
  let payload = SnapshotPayload::new(ArcShared::new(99_i32));

  let bytes = serializer.to_binary(&payload).expect("serialize");
  let restored = serializer.from_binary(&bytes, None).expect("deserialize");
  let restored = restored.downcast_ref::<SnapshotPayload>().expect("snapshot payload");

  assert_eq!(restored.downcast_ref::<i32>(), Some(&99));
}

#[test]
fn snapshot_serializer_reports_manifest_and_rejects_unknown_message_type() {
  let registry = registry();
  let serializer = SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade());

  assert_eq!(serializer.identifier(), SNAPSHOT_SERIALIZER_ID);
  assert!(serializer.include_manifest());
  assert!(serializer.as_any().downcast_ref::<SnapshotSerializer>().is_some());
  assert!(matches!(serializer.to_binary(&"unsupported"), Err(SerializationError::InvalidFormat)));
}

#[test]
fn snapshot_payload_without_manifest_fails_deserialization() {
  let registry = registry();
  let serializer = SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade());
  let nested = SerializedMessage::new(SerializerId::try_from(110).expect("serializer id"), None, Vec::new());

  assert!(matches!(serializer.from_binary(&nested.encode(), None), Err(SerializationError::InvalidFormat)));
}
