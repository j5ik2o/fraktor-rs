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
use fraktor_utils_core_rs::sync::{ArcShared, WeakShared};

use crate::{
  journal::EventAdapters,
  persistent::{AtomicWrite, PersistentRepr},
  serialization::{
    MESSAGE_SERIALIZER_ID, MessageSerializer, PersistenceSerializationContributor, SNAPSHOT_SERIALIZER_ID,
    SnapshotPayload, register_persistence_serializers,
    wire::{self, ATOMIC_WRITE_TAG, PERSISTENT_REPR_TAG},
  },
};

const I32_MANIFEST: &str = "test.I32";

struct DomainEvent;

struct UnboundDomainEvent;

struct UnboundMetadata(i32);

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
  fn manifest(&self, message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    if message.downcast_ref::<i32>() == Some(&i32::MIN) {
      return Cow::Borrowed("");
    }
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
    if let Some(value) = message.downcast_ref::<i32>() {
      return Ok(value.to_le_bytes().to_vec());
    }
    if let Some(value) = message.downcast_ref::<UnboundMetadata>() {
      return Ok(value.0.to_le_bytes().to_vec());
    }
    Err(SerializationError::InvalidFormat)
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    if bytes.len() != core::mem::size_of::<i32>() {
      return Err(SerializationError::InvalidFormat);
    }
    let mut array = [0_u8; core::mem::size_of::<i32>()];
    array.copy_from_slice(bytes);
    let value = i32::from_le_bytes(array);
    if type_hint == Some(TypeId::of::<i32>()) {
      return Ok(Box::new(value));
    }
    if type_hint == Some(TypeId::of::<UnboundMetadata>()) {
      return Ok(Box::new(UnboundMetadata(value)));
    }
    Err(SerializationError::InvalidFormat)
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

fn hint_only_registry_without_payload_binding() -> ArcShared<SerializationRegistry> {
  let id = SerializerId::try_from(101).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(HintOnlyI32Serializer::new(id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("i32", id, serializer)
    .expect("register")
    .set_fallback("i32")
    .expect("fallback")
    .build()
    .expect("setup");
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  register_persistence_serializers(&registry).expect("persistence serializers");
  registry
}

fn fallback_registry_without_payload_binding() -> ArcShared<SerializationRegistry> {
  let id = SerializerId::try_from(100).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(ManifestI32Serializer::new(id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("i32", id, serializer)
    .expect("register")
    .set_fallback("i32")
    .expect("fallback")
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
  assert_eq!(hint_only.to_binary(&UnboundMetadata(2)).expect("metadata"), 2_i32.to_le_bytes().to_vec());
  assert_eq!(
    *hint_only
      .from_binary(&1_i32.to_le_bytes(), Some(TypeId::of::<i32>()))
      .expect("value")
      .downcast_ref::<i32>()
      .unwrap(),
    1
  );
  let metadata = hint_only.from_binary(&2_i32.to_le_bytes(), Some(TypeId::of::<UnboundMetadata>())).expect("metadata");
  assert_eq!(metadata.downcast_ref::<UnboundMetadata>().map(|value| value.0), Some(2));
  assert!(matches!(hint_only.from_binary(&[], None), Err(SerializationError::InvalidFormat)));
  assert!(matches!(hint_only.from_binary(&2_i32.to_le_bytes(), None), Err(SerializationError::InvalidFormat)));
  assert!(matches!(hint_only.to_binary(&"unsupported"), Err(SerializationError::InvalidFormat)));
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
fn persistent_repr_round_trip_uses_payload_type_when_adapter_binding_is_absent() {
  let registry = fallback_registry_without_payload_binding();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 7, ArcShared::new(11_i32));

  let bytes = serializer.to_binary(&repr).expect("serialize");
  let mut cursor = 0;
  assert_eq!(wire::read_u8(&bytes, &mut cursor).expect("tag"), PERSISTENT_REPR_TAG);
  let repr_bytes = wire::read_bytes(&bytes, &mut cursor).expect("repr");
  let mut repr_cursor = 0;
  let _persistence_id = wire::read_string(repr_bytes, &mut repr_cursor).expect("persistence id");
  let _sequence_nr = wire::read_u64(repr_bytes, &mut repr_cursor).expect("sequence nr");
  assert_eq!(wire::read_string(repr_bytes, &mut repr_cursor).expect("payload type"), "");

  let restored = serializer.from_binary(&bytes, None).expect("deserialize");
  let restored = restored.downcast_ref::<PersistentRepr>().expect("persistent repr");

  assert_eq!(restored.downcast_ref::<i32>(), Some(&11));
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
fn unregistered_metadata_fails_serialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let repr =
    PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)).with_metadata(ArcShared::new(String::from("missing")));

  assert!(matches!(serializer.to_binary(&repr), Err(SerializationError::InvalidFormat)));
}

#[test]
fn empty_nested_manifest_payload_fails_serialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(i32::MIN));

  assert!(matches!(serializer.to_binary(&repr), Err(SerializationError::InvalidFormat)));
}

#[test]
fn non_manifest_unbound_payload_fails_serialization() {
  let registry = hint_only_registry_without_payload_binding();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));

  assert!(matches!(serializer.to_binary(&repr), Err(SerializationError::InvalidFormat)));
}

#[test]
fn non_manifest_unbound_metadata_fails_serialization() {
  let registry = hint_only_registry();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)).with_metadata(ArcShared::new(UnboundMetadata(2)));

  assert!(matches!(serializer.to_binary(&repr), Err(SerializationError::InvalidFormat)));
}

#[test]
fn non_manifest_resolvable_payload_serializes_with_serializer_id() {
  let registry = hint_only_registry();
  let serializer = serializer(&registry);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));

  let bytes = serializer.to_binary(&repr).expect("serialize");
  let mut cursor = 0;
  assert_eq!(wire::read_u8(&bytes, &mut cursor).expect("tag"), PERSISTENT_REPR_TAG);
  let repr_bytes = wire::read_bytes(&bytes, &mut cursor).expect("repr");
  let mut repr_cursor = 0;
  let _persistence_id = wire::read_string(repr_bytes, &mut repr_cursor).expect("persistence id");
  let _sequence_nr = wire::read_u64(repr_bytes, &mut repr_cursor).expect("sequence nr");
  let payload_type_name = wire::read_string(repr_bytes, &mut repr_cursor).expect("payload type");
  let nested = wire::read_serialized(repr_bytes, &mut repr_cursor).expect("payload");

  assert_eq!(payload_type_name, "i32");
  assert_eq!(nested.serializer_id(), SerializerId::try_from(101).expect("serializer id"));
  assert_eq!(nested.manifest(), None);

  let restored = serializer.from_binary(&bytes, None).expect("deserialize");
  let restored = restored.downcast_ref::<PersistentRepr>().expect("persistent repr");
  assert_eq!(restored.downcast_ref::<i32>(), Some(&1));
}

#[test]
fn unbound_adapter_type_fails_serialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let repr =
    PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)).with_adapter_type_id(TypeId::of::<UnboundDomainEvent>());

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
fn empty_atomic_write_payload_fails_deserialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let mut bytes = Vec::new();
  wire::write_u8(&mut bytes, ATOMIC_WRITE_TAG);
  wire::write_u32(&mut bytes, 0);

  assert!(matches!(serializer.from_binary(&bytes, None), Err(SerializationError::InvalidFormat)));
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
  wire::write_string(&mut repr, "i32").expect("payload type");
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
fn trailing_nested_payload_bytes_fail_deserialization() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let payload = SerializedMessage::new(
    SerializerId::try_from(100).expect("serializer id"),
    Some(I32_MANIFEST.into()),
    1_i32.to_le_bytes().to_vec(),
  );
  let mut nested = payload.encode();
  nested.push(0);
  let mut repr = Vec::new();
  wire::write_string(&mut repr, "pid-1").expect("persistence id");
  wire::write_u64(&mut repr, 1);
  wire::write_string(&mut repr, "i32").expect("payload type");
  wire::write_bytes(&mut repr, &nested).expect("payload");
  wire::write_string(&mut repr, "").expect("manifest");
  wire::write_string(&mut repr, "").expect("writer uuid");
  wire::write_u64(&mut repr, 0);
  wire::write_bool(&mut repr, false);
  wire::write_string(&mut repr, "").expect("adapter type");
  wire::write_bool(&mut repr, false);
  let mut bytes = Vec::new();
  wire::write_u8(&mut bytes, PERSISTENT_REPR_TAG);
  wire::write_bytes(&mut bytes, &repr).expect("repr");

  assert!(matches!(serializer.from_binary(&bytes, None), Err(SerializationError::InvalidFormat)));
}

#[test]
fn manifest_validation_accepts_missing_manifest() {
  let payload = SerializedMessage::new(SerializerId::try_from(100).expect("serializer id"), None, vec![]);

  assert!(MessageSerializer::has_valid_manifest(&payload));
}

#[test]
fn manifest_validation_rejects_empty_manifest() {
  let payload =
    SerializedMessage::new(SerializerId::try_from(100).expect("serializer id"), Some(String::new()), vec![]);

  assert!(!MessageSerializer::has_valid_manifest(&payload));
}

#[test]
fn persistent_repr_without_nested_manifest_deserializes_by_serializer_id() {
  let registry = manifest_registry();
  let serializer = serializer(&registry);
  let payload =
    SerializedMessage::new(SerializerId::try_from(100).expect("serializer id"), None, 1_i32.to_le_bytes().to_vec());
  let mut repr = Vec::new();
  wire::write_string(&mut repr, "pid-1").expect("persistence id");
  wire::write_u64(&mut repr, 1);
  wire::write_string(&mut repr, "i32").expect("payload type");
  wire::write_serialized(&mut repr, &payload).expect("payload");
  wire::write_string(&mut repr, "").expect("manifest");
  wire::write_string(&mut repr, "").expect("writer uuid");
  wire::write_u64(&mut repr, 0);
  wire::write_bool(&mut repr, false);
  wire::write_string(&mut repr, "").expect("adapter type");
  wire::write_bool(&mut repr, false);
  let mut bytes = Vec::new();
  wire::write_u8(&mut bytes, PERSISTENT_REPR_TAG);
  wire::write_bytes(&mut bytes, &repr).expect("repr");

  let restored = serializer.from_binary(&bytes, None).expect("deserialize");
  let restored = restored.downcast_ref::<PersistentRepr>().expect("persistent repr");

  assert_eq!(restored.downcast_ref::<i32>(), Some(&1));
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
  wire::write_string(&mut repr, "i32").expect("payload type");
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
fn persistence_serializer_registration_rejects_stale_message_serializer_registration() {
  let stale_serializer: ArcShared<dyn Serializer> =
    ArcShared::new(MessageSerializer::new(MESSAGE_SERIALIZER_ID, WeakShared::<SerializationRegistry>::new()));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("persistence-message", MESSAGE_SERIALIZER_ID, stale_serializer)
    .expect("register")
    .set_fallback("persistence-message")
    .expect("fallback")
    .build()
    .expect("setup");
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));

  assert!(matches!(
    register_persistence_serializers(&registry),
    Err(SerializationError::SerializerIdCollision(id)) if id == MESSAGE_SERIALIZER_ID
  ));
}

#[test]
fn persistence_serializer_registration_rejects_message_serializer_with_wrong_internal_id() {
  let fallback_id = SerializerId::try_from(100).expect("serializer id");
  let fallback: ArcShared<dyn Serializer> = ArcShared::new(ManifestI32Serializer::new(fallback_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("fallback", fallback_id, fallback)
    .expect("register")
    .set_fallback("fallback")
    .expect("fallback")
    .build()
    .expect("setup");
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  let wrong_id = SerializerId::try_from(999).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(MessageSerializer::new(wrong_id, registry.downgrade()));

  assert!(registry.register_serializer(MESSAGE_SERIALIZER_ID, serializer));
  assert!(matches!(
    register_persistence_serializers(&registry),
    Err(SerializationError::SerializerIdCollision(id)) if id == MESSAGE_SERIALIZER_ID
  ));
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
  assert!(registry.registered_serializer(MESSAGE_SERIALIZER_ID).is_none());
  assert!(registry.binding_for(TypeId::of::<PersistentRepr>()).is_none());
  assert!(registry.binding_for(TypeId::of::<AtomicWrite>()).is_none());
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

#[test]
fn persistence_serializer_registration_rejects_binding_name_collision() {
  let id = SerializerId::try_from(100).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(ManifestI32Serializer::new(id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("fallback", id, serializer)
    .expect("register")
    .set_fallback("fallback")
    .expect("fallback")
    .build()
    .expect("setup");
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  registry.register_binding(TypeId::of::<String>(), "PersistentRepr", id).expect("runtime binding");

  assert!(matches!(
    register_persistence_serializers(&registry),
    Err(SerializationError::SerializerBindingCollision {
      type_name,
      existing,
      requested
    }) if type_name == "PersistentRepr" && existing == id && requested == MESSAGE_SERIALIZER_ID
  ));
}
