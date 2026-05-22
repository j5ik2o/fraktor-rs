use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_core_kernel_rs::{
  actor::Pid,
  serialization::{
    SerializationDelegator, SerializationError, SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId,
    SerializerWithStringManifest, contribution::SerializationRegistryContributor,
    serialization_registry::SerializationRegistry,
  },
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  journal::EventAdapters,
  persistent::{AtomicWrite, PersistentRepr},
  serialization::{
    MESSAGE_SERIALIZER_ID, MessageSerializer, PersistenceSerializationContributor, SNAPSHOT_SERIALIZER_ID,
    SnapshotPayload, register_persistence_serializers,
    wire::{self, PERSISTENT_REPR_TAG},
  },
};

const I32_MANIFEST: &str = "test.I32";

struct DomainEvent;

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
    .bind::<DomainEvent>("i32")
    .expect("bind domain event")
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
fn test_serializers_exercise_trait_methods() {
  let manifest = ManifestI32Serializer::new(SerializerId::try_from(100).expect("serializer id"));
  assert!(manifest.include_manifest());
  assert!(manifest.as_any().downcast_ref::<ManifestI32Serializer>().is_some());
  assert_eq!(*manifest.from_binary(&1_i32.to_le_bytes(), None).expect("value").downcast_ref::<i32>().unwrap(), 1);
  assert!(matches!(manifest.from_binary_with_manifest(&[], I32_MANIFEST), Err(SerializationError::InvalidFormat)));

  let hint_only = HintOnlyI32Serializer::new(SerializerId::try_from(101).expect("serializer id"));
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
    .with_adapter_type_id(TypeId::of::<DomainEvent>());

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
  assert_eq!(restored.adapter_type_id(), TypeId::of::<DomainEvent>());
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
fn unregistered_metadata_fails_serialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let repr =
    PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)).with_metadata(ArcShared::new(String::from("missing")));

  assert!(matches!(serializer.to_binary(&repr), Err(SerializationError::InvalidFormat)));
}

#[test]
fn non_manifest_resolvable_payload_fails_serialization() {
  let registry = hint_only_registry();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));

  assert!(matches!(serializer.to_binary(&repr), Err(SerializationError::InvalidFormat)));
}

#[test]
fn registry_resolves_persistence_message_serializer() {
  let registry = manifest_registry();
  let delegator = SerializationDelegator::new(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));

  let serialized = delegator.serialize(&repr, "PersistentRepr").expect("serialize");

  assert_eq!(serialized.serializer_id(), MESSAGE_SERIALIZER_ID);
}

#[test]
fn serializer_reports_manifest_and_rejects_unknown_message_type() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);

  assert!(serializer.include_manifest());
  assert!(serializer.as_any().downcast_ref::<MessageSerializer>().is_some());
  assert!(matches!(serializer.to_binary(&"unsupported"), Err(SerializationError::InvalidFormat)));
}

#[test]
fn unknown_wire_tag_fails_deserialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);

  assert!(matches!(serializer.from_binary(&[u8::MAX], None), Err(SerializationError::InvalidFormat)));
}

#[test]
fn empty_manifest_payload_fails_deserialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let payload =
    SerializedMessage::new(SerializerId::try_from(100).expect("serializer id"), Some(String::new()), vec![]);
  let mut repr = Vec::new();
  wire::write_string(&mut repr, "pid-1").expect("persistence id");
  wire::write_u64(&mut repr, 1);
  wire::write_serialized(&mut repr, &payload).expect("payload");
  wire::write_string(&mut repr, "").expect("manifest");
  wire::write_string(&mut repr, "").expect("writer uuid");
  wire::write_u64(&mut repr, 0);
  wire::write_bool(&mut repr, false);
  wire::write_bool(&mut repr, false);
  let mut bytes = Vec::new();
  wire::write_u8(&mut bytes, PERSISTENT_REPR_TAG);
  wire::write_bytes(&mut bytes, &repr).expect("repr");

  assert!(matches!(serializer.from_binary(&bytes, None), Err(SerializationError::InvalidFormat)));
}

#[test]
fn unknown_adapter_type_binding_fails_deserialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let payload = SerializedMessage::new(
    SerializerId::try_from(100).expect("serializer id"),
    Some(I32_MANIFEST.into()),
    1_i32.to_le_bytes().to_vec(),
  );
  let mut repr = Vec::new();
  wire::write_string(&mut repr, "pid-1").expect("persistence id");
  wire::write_u64(&mut repr, 1);
  wire::write_serialized(&mut repr, &payload).expect("payload");
  wire::write_string(&mut repr, "").expect("manifest");
  wire::write_string(&mut repr, "").expect("writer uuid");
  wire::write_u64(&mut repr, 0);
  wire::write_bool(&mut repr, false);
  wire::write_string(&mut repr, "missing-adapter").expect("adapter type");
  wire::write_bool(&mut repr, false);
  let mut bytes = Vec::new();
  wire::write_u8(&mut bytes, PERSISTENT_REPR_TAG);
  wire::write_bytes(&mut bytes, &repr).expect("repr");

  assert!(matches!(serializer.from_binary(&bytes, None), Err(SerializationError::InvalidFormat)));
}

#[test]
fn persistence_serializer_registration_is_idempotent() {
  let registry = manifest_registry();

  let contributor = PersistenceSerializationContributor::default();

  register_persistence_serializers(&registry).expect("register twice");
  contributor.contribute(&registry).expect("contribute twice");
}

#[test]
fn persistence_serializer_registration_rejects_snapshot_binding_collision() {
  let id = SerializerId::try_from(100).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(ManifestI32Serializer::new(id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("snapshot", id, serializer)
    .expect("register")
    .set_fallback("snapshot")
    .expect("fallback")
    .bind::<SnapshotPayload>("snapshot")
    .expect("bind")
    .build()
    .expect("setup");
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));

  assert!(matches!(
    register_persistence_serializers(&registry),
    Err(SerializationError::SerializerBindingCollision {
      type_name,
      existing,
      requested
    }) if type_name == "SnapshotPayload" && existing == id && requested == SNAPSHOT_SERIALIZER_ID
  ));
}

#[test]
fn persistence_serializer_registration_rejects_snapshot_serializer_id_collision() {
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(ManifestI32Serializer::new(SNAPSHOT_SERIALIZER_ID));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("snapshot", SNAPSHOT_SERIALIZER_ID, serializer)
    .expect("register")
    .set_fallback("snapshot")
    .expect("fallback")
    .build()
    .expect("setup");
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));

  assert!(matches!(
    register_persistence_serializers(&registry),
    Err(SerializationError::SerializerIdCollision(id)) if id == SNAPSHOT_SERIALIZER_ID
  ));
}
