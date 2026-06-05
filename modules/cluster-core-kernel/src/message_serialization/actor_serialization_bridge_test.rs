use alloc::{borrow::Cow, boxed::Box, string::String, vec, vec::Vec};
use core::any::{Any, TypeId, type_name};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::{
  serialization::{
    NotSerializableError, SerializationCallScope, SerializationError, SerializationExtension,
    SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId, SerializerWithStringManifest,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::ArcShared;

use super::ActorSerializationBridge;
use crate::message_serialization::{ClusterMessagePayloadKind, ClusterSerializedMessage};

const BRIDGE_MANIFEST: &str = "cluster.bridge.Payload";

#[derive(Debug, PartialEq)]
struct BridgePayload {
  node:  u16,
  value: i16,
}

#[derive(Debug, PartialEq)]
struct RuntimeBoundPayload(u8);

struct BridgeSerializer {
  id: SerializerId,
}

impl BridgeSerializer {
  const fn new(id: SerializerId) -> Self {
    Self { id }
  }

  fn not_serializable(&self, type_name: &str) -> SerializationError {
    SerializationError::not_serializable(NotSerializableError::new(
      type_name,
      Some(self.id),
      Some(String::from(BRIDGE_MANIFEST)),
      None,
      None,
    ))
  }

  fn decode(bytes: &[u8]) -> Result<BridgePayload, SerializationError> {
    if bytes.len() != 4 {
      return Err(SerializationError::invalid_format());
    }
    let node = u16::from_le_bytes(bytes[0..2].try_into().map_err(|_| SerializationError::invalid_format())?);
    let value = i16::from_le_bytes(bytes[2..4].try_into().map_err(|_| SerializationError::invalid_format())?);
    Ok(BridgePayload { node, value })
  }
}

impl Serializer for BridgeSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload =
      message.downcast_ref::<BridgePayload>().ok_or_else(|| self.not_serializable(type_name::<BridgePayload>()))?;
    let mut buffer = Vec::with_capacity(4);
    buffer.extend_from_slice(&payload.node.to_le_bytes());
    buffer.extend_from_slice(&payload.value.to_le_bytes());
    Ok(buffer)
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(Self::decode(bytes)?))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for BridgeSerializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed(BRIDGE_MANIFEST)
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    if manifest != BRIDGE_MANIFEST {
      return Err(SerializationError::unknown_manifest(manifest));
    }
    self.from_binary(bytes, None)
  }
}

struct RuntimeBoundSerializer {
  id: SerializerId,
}

impl RuntimeBoundSerializer {
  const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for RuntimeBoundSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<RuntimeBoundPayload>().ok_or_else(SerializationError::invalid_format)?;
    Ok(vec![payload.0])
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(RuntimeBoundPayload(bytes.first().copied().unwrap_or_default())))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

fn build_bridge_extension() -> (SerializationExtension, SerializerId, SerializerId, ActorSystem) {
  let bridge_id = SerializerId::try_from(501).expect("bridge id");
  let runtime_bound_id = SerializerId::try_from(502).expect("runtime bound id");
  let bridge_serializer: ArcShared<dyn Serializer> = ArcShared::new(BridgeSerializer::new(bridge_id));
  let runtime_bound_serializer: ArcShared<dyn Serializer> =
    ArcShared::new(RuntimeBoundSerializer::new(runtime_bound_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("bridge", bridge_id, bridge_serializer)
    .expect("register bridge")
    .register_serializer("runtime-bound", runtime_bound_id, runtime_bound_serializer)
    .expect("register runtime bound")
    .set_fallback("bridge")
    .expect("fallback")
    .bind::<BridgePayload>("bridge")
    .expect("bind")
    .bind_remote_manifest::<BridgePayload>(BRIDGE_MANIFEST)
    .expect("manifest")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("setup");
  let system = create_noop_actor_system();
  (SerializationExtension::new(&system, setup), bridge_id, runtime_bound_id, system)
}

#[test]
fn serialize_cluster_message_preserves_serializer_id_and_manifest_for_roundtrip() {
  let (extension, serializer_id, _runtime_bound_id, _system) = build_bridge_extension();
  let payload = BridgePayload { node: 7, value: -23 };

  let cluster_message = extension
    .serialize_cluster_message(ClusterMessagePayloadKind::Gossip, SerializationCallScope::Remote, &payload)
    .expect("serialize");

  assert_eq!(cluster_message.payload_kind(), ClusterMessagePayloadKind::Gossip);
  assert_eq!(cluster_message.serializer_id(), serializer_id);
  assert_eq!(cluster_message.manifest(), Some(BRIDGE_MANIFEST));
  assert_eq!(cluster_message.payload_bytes(), &[7, 0, 233, 255]);

  let restored = extension
    .deserialize_cluster_message(&cluster_message, Some(TypeId::of::<BridgePayload>()))
    .expect("deserialize")
    .downcast::<BridgePayload>()
    .expect("payload");
  assert_eq!(*restored, payload);
}

#[test]
fn remote_scope_manifest_requirement_is_not_bypassed() {
  let (extension, _serializer_id, runtime_bound_id, _system) = build_bridge_extension();
  extension
    .register_binding(TypeId::of::<RuntimeBoundPayload>(), type_name::<RuntimeBoundPayload>(), runtime_bound_id)
    .expect("runtime binding");

  let error = extension
    .serialize_cluster_message(
      ClusterMessagePayloadKind::PubSub,
      SerializationCallScope::Remote,
      &RuntimeBoundPayload(1),
    )
    .expect_err("manifest missing");

  assert!(matches!(error, SerializationError::ManifestMissing { scope: SerializationCallScope::Remote }));
}

#[test]
fn serialize_failure_surfaces_as_actor_core_error() {
  let (extension, _serializer_id, _runtime_bound_id, _system) = build_bridge_extension();

  let error = extension
    .serialize_cluster_message(ClusterMessagePayloadKind::Gossip, SerializationCallScope::Remote, &"not bridge payload")
    .expect_err("serialize failure");

  assert!(matches!(error, SerializationError::NotSerializable(_)));
}

#[test]
fn deserialize_failure_surfaces_as_actor_core_error() {
  let (extension, serializer_id, _runtime_bound_id, _system) = build_bridge_extension();
  let serialized = SerializedMessage::new(serializer_id, Some(String::from(BRIDGE_MANIFEST)), vec![1, 2, 3]);
  let cluster_message = ClusterSerializedMessage::new(ClusterMessagePayloadKind::Gossip, serialized);

  let error = extension
    .deserialize_cluster_message(&cluster_message, Some(TypeId::of::<BridgePayload>()))
    .expect_err("deserialize failure");

  assert_eq!(error, SerializationError::InvalidFormat);
}

#[test]
fn unknown_serializer_surfaces_as_actor_core_error() {
  let (extension, _serializer_id, _runtime_bound_id, _system) = build_bridge_extension();
  let unknown_id = SerializerId::try_from(599).expect("unknown id");
  let serialized = SerializedMessage::new(unknown_id, None, Vec::new());
  let cluster_message = ClusterSerializedMessage::new(ClusterMessagePayloadKind::PubSub, serialized);

  let error = extension.deserialize_cluster_message(&cluster_message, None).expect_err("unknown serializer");

  assert_eq!(error, SerializationError::UnknownSerializer(unknown_id));
}
