use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationError, SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId,
  SerializerWithStringManifest, serialization_registry::SerializationRegistry,
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::serialization::{
  SNAPSHOT_SERIALIZER_ID, SnapshotPayload, SnapshotSerializer, register_persistence_serializers, wire,
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

struct EmptyManifestI32Serializer {
  id: SerializerId,
}

impl EmptyManifestI32Serializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for EmptyManifestI32Serializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let value = message.downcast_ref::<i32>().ok_or(SerializationError::InvalidFormat)?;
    Ok(value.to_le_bytes().to_vec())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Err(SerializationError::InvalidFormat)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for EmptyManifestI32Serializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed("")
  }

  fn from_binary_with_manifest(
    &self,
    _bytes: &[u8],
    _manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Err(SerializationError::InvalidFormat)
  }
}

struct HintOnlyI32Serializer {
  id: SerializerId,
}

impl HintOnlyI32Serializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for HintOnlyI32Serializer {
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
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    if type_hint != Some(TypeId::of::<i32>()) || bytes.len() != core::mem::size_of::<i32>() {
      return Err(SerializationError::InvalidFormat);
    }
    let mut array = [0_u8; core::mem::size_of::<i32>()];
    array.copy_from_slice(bytes);
    Ok(Box::new(i32::from_le_bytes(array)))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
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

fn hint_only_registry() -> ArcShared<SerializationRegistry> {
  let id = SerializerId::try_from(111).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(HintOnlyI32Serializer::new(id));
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

fn empty_manifest_registry() -> ArcShared<SerializationRegistry> {
  let id = SerializerId::try_from(112).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(EmptyManifestI32Serializer::new(id));
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
fn test_serializers_exercise_trait_methods() {
  let manifest = ManifestI32Serializer::new(SerializerId::try_from(110).expect("serializer id"));
  assert!(manifest.as_any().downcast_ref::<ManifestI32Serializer>().is_some());
  assert_eq!(*manifest.from_binary(&1_i32.to_le_bytes(), None).expect("value").downcast_ref::<i32>().unwrap(), 1);
  assert!(matches!(manifest.from_binary_with_manifest(&[], I32_MANIFEST), Err(SerializationError::InvalidFormat)));

  let hint_only = HintOnlyI32Serializer::new(SerializerId::try_from(111).expect("serializer id"));
  assert!(hint_only.as_any().downcast_ref::<HintOnlyI32Serializer>().is_some());
  assert_eq!(
    *hint_only
      .from_binary(&1_i32.to_le_bytes(), Some(TypeId::of::<i32>()))
      .expect("value")
      .downcast_ref::<i32>()
      .unwrap(),
    1
  );
  assert!(matches!(hint_only.from_binary(&[], None), Err(SerializationError::InvalidFormat)));
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
fn snapshot_payload_without_manifest_deserializes_by_serializer_id() {
  let registry = registry();
  let serializer = SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade());
  let nested =
    SerializedMessage::new(SerializerId::try_from(110).expect("serializer id"), None, 1_i32.to_le_bytes().to_vec());
  let mut bytes = Vec::new();
  wire::write_string(&mut bytes, "i32").expect("payload type");
  wire::write_serialized(&mut bytes, &nested).expect("payload");

  let restored = serializer.from_binary(&bytes, None).expect("deserialize");
  let restored = restored.downcast_ref::<SnapshotPayload>().expect("snapshot payload");

  assert_eq!(restored.downcast_ref::<i32>(), Some(&1));
}

#[test]
fn snapshot_payload_with_empty_manifest_fails_deserialization() {
  let registry = registry();
  let serializer = SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade());
  let nested =
    SerializedMessage::new(SerializerId::try_from(110).expect("serializer id"), Some(String::new()), Vec::new());
  let mut bytes = Vec::new();
  wire::write_string(&mut bytes, "i32").expect("payload type");
  wire::write_serialized(&mut bytes, &nested).expect("payload");

  assert!(matches!(serializer.from_binary(&bytes, None), Err(SerializationError::InvalidFormat)));
}

#[test]
fn snapshot_payload_with_trailing_nested_bytes_fails_deserialization() {
  let registry = registry();
  let serializer = SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade());
  let nested = SerializedMessage::new(
    SerializerId::try_from(110).expect("serializer id"),
    Some(I32_MANIFEST.into()),
    1_i32.to_le_bytes().to_vec(),
  );
  let mut encoded = nested.encode();
  encoded.push(0);
  let mut bytes = Vec::new();
  wire::write_string(&mut bytes, "i32").expect("payload type");
  wire::write_bytes(&mut bytes, &encoded).expect("payload");

  assert!(matches!(serializer.from_binary(&bytes, None), Err(SerializationError::InvalidFormat)));
}

#[test]
fn snapshot_payload_without_manifest_serializes_with_serializer_id() {
  let registry = hint_only_registry();
  let serializer = SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade());
  let payload = SnapshotPayload::new(ArcShared::new(99_i32));

  let bytes = serializer.to_binary(&payload).expect("serialize");
  let mut cursor = 0;
  let payload_type_name = wire::read_string(&bytes, &mut cursor).expect("payload type");
  let nested = wire::read_serialized(&bytes, &mut cursor).expect("decode");

  assert_eq!(payload_type_name, "i32");
  assert_eq!(nested.serializer_id(), SerializerId::try_from(111).expect("serializer id"));
  assert_eq!(nested.manifest(), None);

  let restored = serializer.from_binary(&bytes, None).expect("deserialize");
  let restored = restored.downcast_ref::<SnapshotPayload>().expect("snapshot payload");
  assert_eq!(restored.downcast_ref::<i32>(), Some(&99));
}

#[test]
fn snapshot_payload_with_empty_nested_manifest_fails_serialization() {
  let registry = empty_manifest_registry();
  let serializer = SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade());
  let payload = SnapshotPayload::new(ArcShared::new(99_i32));

  assert!(matches!(serializer.to_binary(&payload), Err(SerializationError::InvalidFormat)));
}
