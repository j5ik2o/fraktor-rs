#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::{borrow::Cow, string::String, vec::Vec};
use core::{
  any::{Any, TypeId},
  convert::{TryFrom, TryInto},
};

use fraktor_actor_core_rs::core::{
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::Props,
  serialization::{
    NotSerializableError, SerializationCallScope, SerializationError, SerializationExtension, SerializationExtensionId,
    SerializationSetup, SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId,
    SerializerWithStringManifest, TransportInformation,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::ArcShared;

const TELEMETRY_MANIFEST: &str = "sample.telemetry.TelemetryPayload";
const SERIALIZER_NAME: &str = "telemetry";

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
      Some(TELEMETRY_MANIFEST.into()),
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

  fn from_binary(&self, bytes: &[u8], _type_hint: Option<TypeId>) -> Result<Box<dyn Any + Send>, SerializationError> {
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
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    self.from_binary(bytes, None)
  }
}

struct NullActor;

impl NullActor {
  fn new() -> Self {
    Self
  }
}

impl Actor for NullActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageViewGeneric<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_serialization_setup(serializer_id: SerializerId) -> SerializationSetup {
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(TelemetrySerializer::new(serializer_id));
  SerializationSetupBuilder::new()
    .register_serializer(SERIALIZER_NAME, serializer_id, serializer)
    .expect("register serializer")
    .set_fallback(SERIALIZER_NAME)
    .expect("set fallback")
    .bind::<TelemetryPayload>(SERIALIZER_NAME)
    .expect("bind type")
    .bind_remote_manifest::<TelemetryPayload>(TELEMETRY_MANIFEST)
    .expect("bind manifest")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("build setup")
}

#[cfg(not(target_os = "none"))]
fn main() {
  // セットアップ用のシリアライザ ID を確保
  let serializer_id = SerializerId::try_from(200).expect("valid serializer id");
  let setup = build_serialization_setup(serializer_id);
  let serialization_id = SerializationExtensionId::new(setup);
  let configure_id = serialization_id.clone();

  let props = Props::from_fn(NullActor::new).with_name("serialization-demo");
  let system = ActorSystem::new_with(&props, move |system| {
    // ActorSystem 起動前にシリアライゼーション拡張を登録
    let _ = system.register_extension(&configure_id);
    Ok(())
  })
  .expect("actor system");

  let serialization: ArcShared<SerializationExtension> =
    system.extension(&serialization_id).expect("extension registered");

  let payload = TelemetryPayload { node: 7, temperature: 24 };
  let serialized: SerializedMessage =
    serialization.serialize(&payload, SerializationCallScope::Remote).expect("serialize remote");

  println!("manifest: {:?}", serialized.manifest());
  println!("bytes: {:?}", serialized.bytes());

  let decoded_any =
    serialization.deserialize(&serialized, Some(TypeId::of::<TelemetryPayload>())).expect("deserialize");
  let restored = *decoded_any.downcast::<TelemetryPayload>().expect("downcast payload");
  println!("restored payload: node={}, temperature={}", restored.node, restored.temperature);

  // TransportInformation を付与すると、ActorRef からリモート経路形式を作れる
  let info = TransportInformation::new(Some(String::from("fraktor://sample@localhost:2552")));
  serialization.with_transport_information(info, || {
    let path = serialization.serialized_actor_path(&system.user_guardian_ref()).expect("actor path");
    println!("serialized actor path: {path}");
  });

  system.terminate().expect("terminate");
  system.run_until_terminated();
}

#[cfg(target_os = "none")]
fn main() {}
