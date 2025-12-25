use alloc::{borrow::Cow, string::String, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_rs::core::{
  messaging::AnyMessageGeneric,
  serialization::{
    NotSerializableError, SerializationCallScope, SerializationError, SerializationExtensionId,
    SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId, SerializerWithStringManifest,
  },
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{GrainCodec, GrainCodecError, SerializationGrainCodec};

const TELEMETRY_MANIFEST: &str = "sample.telemetry.TelemetryPayload";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TelemetryPayload {
  node:        u16,
  temperature: i16,
}

struct TelemetrySerializer {
  id: SerializerId,
}

impl TelemetrySerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }

  fn payload_error(&self) -> SerializationError {
    SerializationError::not_serializable(NotSerializableError::new(
      "TelemetryPayload",
      Some(self.id),
      Some(String::from(TELEMETRY_MANIFEST)),
      None,
      None,
    ))
  }

  fn decode(bytes: &[u8]) -> Result<TelemetryPayload, SerializationError> {
    if bytes.len() != 4 {
      return Err(SerializationError::invalid_format());
    }
    let node = u16::from_le_bytes(bytes[0..2].try_into().map_err(|_| SerializationError::invalid_format())?);
    let temperature = i16::from_le_bytes(bytes[2..4].try_into().map_err(|_| SerializationError::invalid_format())?);
    Ok(TelemetryPayload { node, temperature })
  }
}

impl Serializer for TelemetrySerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<TelemetryPayload>().ok_or_else(|| self.payload_error())?;
    let mut buffer = Vec::with_capacity(4);
    buffer.extend_from_slice(&payload.node.to_le_bytes());
    buffer.extend_from_slice(&payload.temperature.to_le_bytes());
    Ok(buffer)
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let payload = Self::decode(bytes)?;
    Ok(Box::new(payload))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for TelemetrySerializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed(TELEMETRY_MANIFEST)
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    _manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    self.from_binary(bytes, None)
  }
}

#[test]
fn try_from_system_fails_when_extension_missing() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  match SerializationGrainCodec::try_from_system(&system, SerializationCallScope::Remote) {
    | Ok(_) => panic!("extension should be missing"),
    | Err(err) => assert!(matches!(err, GrainCodecError::ExtensionUnavailable { .. })),
  }
}

#[test]
fn encode_decode_roundtrip_with_custom_serializer() {
  let system = build_system_with_serialization();
  let codec = SerializationGrainCodec::try_from_system(&system, SerializationCallScope::Remote).expect("codec");
  let payload = TelemetryPayload { node: 7, temperature: 24 };
  let message = AnyMessageGeneric::new(payload);

  let encoded = codec.encode(&message).expect("encode");
  let decoded = codec.decode(&encoded).expect("decode");
  let boxed = decoded.payload().downcast_ref::<Box<dyn Any + Send + Sync>>().expect("payload");
  let restored = boxed.as_ref().downcast_ref::<TelemetryPayload>().expect("payload");
  assert_eq!(restored, &payload);
}

#[test]
fn encode_returns_error_when_serializer_unregistered() {
  let system = build_system_with_serialization();
  let codec = SerializationGrainCodec::try_from_system(&system, SerializationCallScope::Remote).expect("codec");
  let message = AnyMessageGeneric::new("unregistered");

  let err = codec.encode(&message).expect_err("encode failure");
  assert!(matches!(err, GrainCodecError::SerializerNotRegistered { .. }));
}

#[test]
fn decode_returns_error_on_incompatible_payload() {
  let system = build_system_with_serialization();
  let codec = SerializationGrainCodec::try_from_system(&system, SerializationCallScope::Remote).expect("codec");

  let serializer_id = SerializerId::try_from(200).expect("serializer id");
  let message = SerializedMessage::new(serializer_id, Some(String::from(TELEMETRY_MANIFEST)), vec![1, 2, 3]);

  let err = codec.decode(&message).expect_err("decode failure");
  assert!(matches!(err, GrainCodecError::Incompatible { .. }));
}

fn build_system_with_serialization() -> ActorSystemGeneric<NoStdToolbox> {
  let serializer_id = SerializerId::try_from(200).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(TelemetrySerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("telemetry", serializer_id, serializer)
    .expect("register serializer")
    .set_fallback("telemetry")
    .expect("fallback")
    .bind::<TelemetryPayload>("telemetry")
    .expect("bind payload")
    .bind_remote_manifest::<TelemetryPayload>(TELEMETRY_MANIFEST)
    .expect("manifest")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("setup");
  let extension_id = SerializationExtensionId::new(setup);

  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  system.extended().register_extension(&extension_id).expect("register extension");
  system
}
