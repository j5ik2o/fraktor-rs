use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_core_kernel_rs::{
  actor::Pid,
  serialization::{
    SerializationDelegator, SerializationError, SerializationSetupBuilder, Serializer, SerializerId,
    SerializerWithStringManifest, serialization_registry::SerializationRegistry,
  },
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  journal::EventAdapters,
  persistent::{AtomicWrite, PersistentRepr},
  serialization::{MESSAGE_SERIALIZER_ID, MessageSerializer, register_persistence_serializers},
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

  fn include_manifest(&self) -> bool {
    true
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

  fn include_manifest(&self) -> bool {
    true
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
    if bytes.len() != core::mem::size_of::<i32>() {
      return Err(SerializationError::InvalidFormat);
    }
    let mut array = [0_u8; core::mem::size_of::<i32>()];
    array.copy_from_slice(bytes);
    Ok(Box::new(i32::from_le_bytes(array)))
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
    bytes: &[u8],
    _manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    self.from_binary(bytes, Some(TypeId::of::<i32>()))
  }
}

fn manifest_registry() -> ArcShared<SerializationRegistry> {
  let id = SerializerId::try_from(100).expect("serializer id");
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

fn empty_manifest_registry() -> ArcShared<SerializationRegistry> {
  let id = SerializerId::try_from(102).expect("serializer id");
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

fn hint_only_registry() -> ArcShared<SerializationRegistry> {
  let id = SerializerId::try_from(101).expect("serializer id");
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

fn serializer(registry: &ArcShared<SerializationRegistry>) -> MessageSerializer {
  MessageSerializer::new(MESSAGE_SERIALIZER_ID, registry.downgrade())
}

#[test]
fn persistent_repr_round_trip_preserves_durable_metadata() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 7, ArcShared::new(11_i32))
    .with_metadata(ArcShared::new(13_i32))
    .with_manifest("event-manifest")
    .with_writer_uuid("writer")
    .with_timestamp(123)
    .with_deleted(true)
    .with_sender(Some(Pid::new(1, 1)))
    .with_adapters(EventAdapters::new())
    .with_adapter_type_id(TypeId::of::<String>());

  let bytes = serializer.to_binary(&repr).expect("serialize");
  let restored = serializer.from_binary(&bytes, None).expect("deserialize");
  let restored = restored.downcast_ref::<PersistentRepr>().expect("persistent repr");

  assert_eq!(restored.persistence_id(), "pid-1");
  assert_eq!(restored.sequence_nr(), 7);
  assert_eq!(restored.manifest(), "event-manifest");
  assert_eq!(restored.writer_uuid(), "writer");
  assert_eq!(restored.timestamp(), 123);
  assert!(restored.deleted());
  assert_eq!(restored.downcast_ref::<i32>(), Some(&11));
  assert_eq!(restored.metadata().and_then(|value| value.downcast_ref::<i32>()), Some(&13));
  assert_eq!(restored.sender(), None);
  assert_eq!(restored.adapter_type_id(), TypeId::of::<i32>());
}

#[test]
fn atomic_write_round_trip_restores_payloads() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let repr1 = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));
  let repr2 = PersistentRepr::new("pid-1", 2, ArcShared::new(2_i32));
  let atomic_write = AtomicWrite::new(vec![repr1, repr2]).expect("atomic write");

  let bytes = serializer.to_binary(&atomic_write).expect("serialize");
  let restored = serializer.from_binary(&bytes, None).expect("deserialize");
  let restored = restored.downcast_ref::<AtomicWrite>().expect("atomic write");

  assert_eq!(restored.persistence_id(), "pid-1");
  assert_eq!(restored.size(), 2);
  assert_eq!(restored.payload()[1].downcast_ref::<i32>(), Some(&2));
}

#[test]
fn unregistered_payload_fails_serialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(String::from("missing")));

  assert!(serializer.to_binary(&repr).is_err());
}

#[test]
fn non_manifest_resolvable_payload_fails_deserialization() {
  let registry = hint_only_registry();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));

  let bytes = serializer.to_binary(&repr).expect("serialize");

  assert!(serializer.from_binary(&bytes, None).is_err());
}

#[test]
fn empty_manifest_payload_fails_deserialization() {
  let registry = empty_manifest_registry();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));

  let bytes = serializer.to_binary(&repr).expect("serialize");

  assert!(matches!(serializer.from_binary(&bytes, None), Err(SerializationError::InvalidFormat)));
}

#[test]
fn registry_resolves_persistence_message_serializer() {
  let registry = manifest_registry();
  let delegator = SerializationDelegator::new(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));

  let serialized = delegator.serialize(&repr, "PersistentRepr").expect("serialize");

  assert_eq!(serialized.serializer_id(), MESSAGE_SERIALIZER_ID);
}
