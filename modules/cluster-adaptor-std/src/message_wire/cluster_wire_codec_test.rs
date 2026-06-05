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
use fraktor_cluster_core_kernel_rs::{
  membership::{GossipEnvelope, GossipPayloadKind, MembershipVersion},
  message_serialization::{ActorSerializationBridge, ClusterMessagePayloadKind, ClusterSerializedMessage},
  pub_sub::{PubSubGossipHandoff, TopicRegistryGossipPayload, TopicRegistryStatus, TopicRegistryVersion},
};
use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::sync::ArcShared;
use postcard::{from_bytes, to_allocvec};
use serde::{Deserialize, Serialize};

use super::ClusterWireCodec;
use crate::message_wire::{ClusterWireDecodeFailure, ClusterWireFrameV1};

const GOSSIP_MANIFEST: &str = "cluster.payload/GossipEnvelope";
const PUBSUB_MANIFEST: &str = "cluster.payload/PubSubGossipHandoff";

#[derive(Debug, Deserialize, Serialize)]
struct GossipEnvelopeRecord {
  from_system:        String,
  from_host:          String,
  from_port:          u16,
  from_uid:           u64,
  to_system:          String,
  to_host:            String,
  to_port:            u16,
  to_uid:             u64,
  payload_kind_tag:   u8,
  membership_version: u64,
  deadline_tick:      u64,
}

impl GossipEnvelopeRecord {
  fn from_envelope(envelope: &GossipEnvelope) -> Self {
    Self {
      from_system:        String::from(envelope.from().address().system()),
      from_host:          String::from(envelope.from().address().host()),
      from_port:          envelope.from().address().port(),
      from_uid:           envelope.from().uid(),
      to_system:          String::from(envelope.to().address().system()),
      to_host:            String::from(envelope.to().address().host()),
      to_port:            envelope.to().address().port(),
      to_uid:             envelope.to().uid(),
      payload_kind_tag:   payload_kind_tag(envelope.payload_kind()),
      membership_version: envelope.membership_version().value(),
      deadline_tick:      envelope.deadline_tick(),
    }
  }

  fn into_envelope(self) -> Result<GossipEnvelope, SerializationError> {
    let from = UniqueAddress::new(Address::new(self.from_system, self.from_host, self.from_port), self.from_uid);
    let to = UniqueAddress::new(Address::new(self.to_system, self.to_host, self.to_port), self.to_uid);
    let payload_kind = payload_kind_from_tag(self.payload_kind_tag)?;
    GossipEnvelope::try_new(from, to, payload_kind, MembershipVersion::new(self.membership_version), self.deadline_tick)
      .map_err(|_| SerializationError::invalid_format())
  }
}

#[derive(Debug, Deserialize, Serialize)]
struct PubSubOwnerVersionRecord {
  system:  String,
  host:    String,
  port:    u16,
  uid:     u64,
  version: u64,
}

impl PubSubOwnerVersionRecord {
  fn from_owner_version(owner: &UniqueAddress, version: TopicRegistryVersion) -> Self {
    Self {
      system:  String::from(owner.address().system()),
      host:    String::from(owner.address().host()),
      port:    owner.address().port(),
      uid:     owner.uid(),
      version: version.value(),
    }
  }

  fn into_owner_version(self) -> (UniqueAddress, TopicRegistryVersion) {
    (
      UniqueAddress::new(Address::new(self.system, self.host, self.port), self.uid),
      TopicRegistryVersion::new(self.version),
    )
  }
}

#[derive(Debug, Deserialize, Serialize)]
struct PubSubGossipHandoffRecord {
  payload_kind_tag: u8,
  owner_versions:   Vec<PubSubOwnerVersionRecord>,
}

impl PubSubGossipHandoffRecord {
  fn from_handoff(handoff: &PubSubGossipHandoff) -> Result<Self, SerializationError> {
    let TopicRegistryGossipPayload::Status(status) = handoff.payload() else {
      return Err(SerializationError::invalid_format());
    };
    Ok(Self {
      payload_kind_tag: payload_kind_tag(handoff.payload_kind()),
      owner_versions:   status
        .owner_versions()
        .iter()
        .map(|(owner, version)| PubSubOwnerVersionRecord::from_owner_version(owner, *version))
        .collect(),
    })
  }

  fn into_handoff(self) -> Result<PubSubGossipHandoff, SerializationError> {
    let payload_kind = payload_kind_from_tag(self.payload_kind_tag)?;
    if payload_kind != GossipPayloadKind::PubSubRegistryStatus {
      return Err(SerializationError::invalid_format());
    }
    Ok(PubSubGossipHandoff::status(TopicRegistryStatus::new(
      self.owner_versions.into_iter().map(PubSubOwnerVersionRecord::into_owner_version).collect(),
    )))
  }
}

struct GossipEnvelopeSerializer {
  id: SerializerId,
}

struct PubSubGossipHandoffSerializer {
  id: SerializerId,
}

impl GossipEnvelopeSerializer {
  const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl PubSubGossipHandoffSerializer {
  const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for GossipEnvelopeSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let envelope = message.downcast_ref::<GossipEnvelope>().ok_or_else(|| {
      SerializationError::not_serializable(NotSerializableError::new(
        type_name::<GossipEnvelope>(),
        Some(self.id),
        Some(String::from(GOSSIP_MANIFEST)),
        None,
        None,
      ))
    })?;
    to_allocvec(&GossipEnvelopeRecord::from_envelope(envelope)).map_err(|_| SerializationError::invalid_format())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let record: GossipEnvelopeRecord = from_bytes(bytes).map_err(|_| SerializationError::invalid_format())?;
    Ok(Box::new(record.into_envelope()?))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl Serializer for PubSubGossipHandoffSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let handoff = message.downcast_ref::<PubSubGossipHandoff>().ok_or_else(|| {
      SerializationError::not_serializable(NotSerializableError::new(
        type_name::<PubSubGossipHandoff>(),
        Some(self.id),
        Some(String::from(PUBSUB_MANIFEST)),
        None,
        None,
      ))
    })?;
    to_allocvec(&PubSubGossipHandoffRecord::from_handoff(handoff)?).map_err(|_| SerializationError::invalid_format())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let record: PubSubGossipHandoffRecord = from_bytes(bytes).map_err(|_| SerializationError::invalid_format())?;
    Ok(Box::new(record.into_handoff()?))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for GossipEnvelopeSerializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed(GOSSIP_MANIFEST)
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    if manifest != GOSSIP_MANIFEST {
      return Err(SerializationError::unknown_manifest(manifest));
    }
    self.from_binary(bytes, None)
  }
}

impl SerializerWithStringManifest for PubSubGossipHandoffSerializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed(PUBSUB_MANIFEST)
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    if manifest != PUBSUB_MANIFEST {
      return Err(SerializationError::unknown_manifest(manifest));
    }
    self.from_binary(bytes, None)
  }
}

const fn payload_kind_tag(kind: GossipPayloadKind) -> u8 {
  match kind {
    | GossipPayloadKind::Delta => 0,
    | GossipPayloadKind::FullState => 1,
    | GossipPayloadKind::SeenDigest => 2,
    | GossipPayloadKind::HeartbeatRequest => 3,
    | GossipPayloadKind::HeartbeatResponse => 4,
    | GossipPayloadKind::CrossDcHeartbeat => 5,
    | GossipPayloadKind::PubSubRegistryStatus => 6,
    | GossipPayloadKind::PubSubRegistryDelta => 7,
  }
}

const fn payload_kind_from_tag(tag: u8) -> Result<GossipPayloadKind, SerializationError> {
  match tag {
    | 0 => Ok(GossipPayloadKind::Delta),
    | 1 => Ok(GossipPayloadKind::FullState),
    | 2 => Ok(GossipPayloadKind::SeenDigest),
    | 3 => Ok(GossipPayloadKind::HeartbeatRequest),
    | 4 => Ok(GossipPayloadKind::HeartbeatResponse),
    | 5 => Ok(GossipPayloadKind::CrossDcHeartbeat),
    | 6 => Ok(GossipPayloadKind::PubSubRegistryStatus),
    | 7 => Ok(GossipPayloadKind::PubSubRegistryDelta),
    | _ => Err(SerializationError::invalid_format()),
  }
}

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

fn gossip_envelope() -> GossipEnvelope {
  GossipEnvelope::try_new(
    unique_address("node-a", 10),
    unique_address("node-b", 11),
    GossipPayloadKind::FullState,
    MembershipVersion::new(7),
    100,
  )
  .expect("confirmed identities should build an envelope")
}

fn pubsub_status_handoff() -> PubSubGossipHandoff {
  PubSubGossipHandoff::status(TopicRegistryStatus::new(vec![
    (unique_address("node-a", 20), TopicRegistryVersion::new(3)),
    (unique_address("node-b", 21), TopicRegistryVersion::new(5)),
  ]))
}

fn build_gossip_bridge_extension() -> (SerializationExtension, SerializerId, ActorSystem) {
  let serializer_id = SerializerId::try_from(601).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(GossipEnvelopeSerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("gossip-envelope", serializer_id, serializer)
    .expect("register gossip serializer")
    .set_fallback("gossip-envelope")
    .expect("fallback")
    .bind::<GossipEnvelope>("gossip-envelope")
    .expect("bind gossip envelope")
    .bind_remote_manifest::<GossipEnvelope>(GOSSIP_MANIFEST)
    .expect("manifest")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("setup");
  let system = create_noop_actor_system();
  (SerializationExtension::new(&system, setup), serializer_id, system)
}

fn build_pubsub_bridge_extension() -> (SerializationExtension, SerializerId, ActorSystem) {
  let serializer_id = SerializerId::try_from(602).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(PubSubGossipHandoffSerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("pubsub-handoff", serializer_id, serializer)
    .expect("register pubsub serializer")
    .set_fallback("pubsub-handoff")
    .expect("fallback")
    .bind::<PubSubGossipHandoff>("pubsub-handoff")
    .expect("bind pubsub handoff")
    .bind_remote_manifest::<PubSubGossipHandoff>(PUBSUB_MANIFEST)
    .expect("manifest")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("setup");
  let system = create_noop_actor_system();
  (SerializationExtension::new(&system, setup), serializer_id, system)
}

fn cluster_message_with_manifest(manifest: Option<String>, payload_bytes: Vec<u8>) -> ClusterSerializedMessage {
  let serialized_message = SerializedMessage::new(SerializerId::from_raw(41), manifest, payload_bytes);
  ClusterSerializedMessage::new(ClusterMessagePayloadKind::Gossip, serialized_message)
}

fn cluster_message() -> ClusterSerializedMessage {
  cluster_message_with_manifest(Some(String::from("cluster.payload/gossip")), vec![1, 1, 2, 3, 5, 8])
}

fn encoded_frame(message: &ClusterSerializedMessage) -> Vec<u8> {
  let frame = ClusterWireFrameV1::try_from_cluster_serialized_message(message).expect("frame");
  to_allocvec(&frame).expect("encode frame")
}

#[test]
fn decode_roundtrip_preserves_cluster_serialized_metadata() {
  let codec = ClusterWireCodec;
  let message = cluster_message();

  let encoded = codec.encode(&message).expect("encode message");
  let decoded = codec.decode(&encoded).expect("decode message");

  assert_eq!(decoded.payload_kind(), ClusterMessagePayloadKind::Gossip);
  assert_eq!(decoded.serializer_id().value(), 41);
  assert_eq!(decoded.manifest(), Some("cluster.payload/gossip"));
  assert_eq!(decoded.payload_bytes(), &[1, 1, 2, 3, 5, 8]);
}

#[test]
fn unsupported_frame_version_returns_unknown_version() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded[0] = 2;

  let failure = codec.decode(&encoded).expect_err("unknown version");

  assert_eq!(failure, ClusterWireDecodeFailure::UnknownVersion);
}

#[test]
fn unsupported_frame_version_with_trailing_bytes_returns_unknown_version() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded[0] = 2;
  encoded.extend_from_slice(&[0x01, 0x02]);

  let failure = codec.decode(&encoded).expect_err("unknown version");

  assert_eq!(failure, ClusterWireDecodeFailure::UnknownVersion);
}

#[test]
fn unknown_payload_kind_tag_returns_unknown_payload_kind() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded[1] = 99;

  let failure = codec.decode(&encoded).expect_err("unknown payload kind");

  assert_eq!(failure, ClusterWireDecodeFailure::UnknownPayloadKind);
}

#[test]
fn payload_length_mismatch_returns_malformed_payload() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message_with_manifest(None, vec![1, 2, 3]));
  encoded[4] = 4;

  let failure = codec.decode(&encoded).expect_err("payload length mismatch");

  assert_eq!(failure, ClusterWireDecodeFailure::MalformedPayload);
}

#[test]
fn invalid_manifest_bytes_returns_malformed_payload() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message_with_manifest(Some(String::from("a")), Vec::new()));
  let manifest_byte = encoded.iter().position(|byte| *byte == b'a').expect("manifest byte");
  encoded[manifest_byte] = 0xff;

  let failure = codec.decode(&encoded).expect_err("invalid manifest bytes");

  assert_eq!(failure, ClusterWireDecodeFailure::MalformedPayload);
}

#[test]
fn invalid_postcard_bytes_return_malformed_payload() {
  let codec = ClusterWireCodec;
  let encoded = [0xff];

  let failure = codec.decode(&encoded).expect_err("invalid postcard bytes");

  assert_eq!(failure, ClusterWireDecodeFailure::MalformedPayload);
}

#[test]
fn trailing_bytes_return_malformed_payload() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded.extend_from_slice(&[0x01, 0x02]);

  let failure = codec.decode(&encoded).expect_err("trailing bytes");

  assert_eq!(failure, ClusterWireDecodeFailure::MalformedPayload);
}

#[test]
fn decode_failure_returns_error_without_fallback_message() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded[1] = 99;

  let result = codec.decode(&encoded);

  assert!(matches!(result, Err(ClusterWireDecodeFailure::UnknownPayloadKind)));
}

#[test]
fn gossip_payload_bridge_roundtrip_does_not_evaluate_gossip_semantics() {
  let (extension, serializer_id, _system) = build_gossip_bridge_extension();
  let codec = ClusterWireCodec;
  let payload = gossip_envelope();

  let cluster_message = extension
    .serialize_cluster_message(ClusterMessagePayloadKind::Gossip, SerializationCallScope::Remote, &payload)
    .expect("serialize gossip payload");
  let encoded = codec.encode(&cluster_message).expect("encode gossip payload");
  let decoded = codec.decode(&encoded).expect("decode gossip payload");
  let restored = extension
    .deserialize_cluster_message(&decoded, Some(TypeId::of::<GossipEnvelope>()))
    .expect("deserialize gossip payload")
    .downcast::<GossipEnvelope>()
    .expect("gossip envelope");

  assert_eq!(cluster_message.payload_kind(), ClusterMessagePayloadKind::Gossip);
  assert_eq!(decoded.payload_kind(), ClusterMessagePayloadKind::Gossip);
  assert_eq!(decoded.serializer_id(), serializer_id);
  assert_eq!(decoded.manifest(), Some(GOSSIP_MANIFEST));
  assert_eq!(*restored, payload);
}

#[test]
fn pubsub_payload_bridge_roundtrip_does_not_mutate_mediator_state() {
  let (extension, serializer_id, _system) = build_pubsub_bridge_extension();
  let codec = ClusterWireCodec;
  let payload = pubsub_status_handoff();

  let cluster_message = extension
    .serialize_cluster_message(ClusterMessagePayloadKind::PubSub, SerializationCallScope::Remote, &payload)
    .expect("serialize pubsub payload");
  let encoded = codec.encode(&cluster_message).expect("encode pubsub payload");
  let decoded = codec.decode(&encoded).expect("decode pubsub payload");
  let restored = extension
    .deserialize_cluster_message(&decoded, Some(TypeId::of::<PubSubGossipHandoff>()))
    .expect("deserialize pubsub payload")
    .downcast::<PubSubGossipHandoff>()
    .expect("pubsub handoff");

  assert_eq!(cluster_message.payload_kind(), ClusterMessagePayloadKind::PubSub);
  assert_eq!(decoded.payload_kind(), ClusterMessagePayloadKind::PubSub);
  assert_eq!(decoded.serializer_id(), serializer_id);
  assert_eq!(decoded.manifest(), Some(PUBSUB_MANIFEST));
  assert_eq!(*restored, payload);
}
