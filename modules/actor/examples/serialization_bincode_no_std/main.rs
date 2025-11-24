#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use alloc::{borrow::Cow, string::String, vec::Vec};
use core::{
  any::{Any, TypeId},
  convert::TryFrom,
};

use bincode::config;
use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::Props,
  serialization::{
    NotSerializableError, SerializationCallScope, SerializationError, SerializationExtension, SerializationExtensionId,
    SerializationSetup, SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId,
    SerializerWithStringManifest, TransportInformation,
  },
  system::{ActorSystem, ActorSystemConfig},
};
use fraktor_utils_rs::core::sync::ArcShared;
use serde::{Deserialize, Serialize};

const TELEMETRY_MANIFEST: &str = "sample.telemetry.TelemetryPayload";
const SERIALIZER_NAME: &str = "telemetry-bincode";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct TelemetryPayload {
  node:        u16,
  temperature: i16,
}

struct BincodeTelemetrySerializer {
  id: SerializerId,
}

impl BincodeTelemetrySerializer {
  const fn new(id: SerializerId) -> Self {
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
}

impl Serializer for BincodeTelemetrySerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<TelemetryPayload>().ok_or_else(|| self.payload_error())?;
    let config = config::standard();
    bincode::serde::encode_to_vec(payload, config).map_err(|_| SerializationError::invalid_format())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let config = config::standard();
    let (payload, _len): (TelemetryPayload, usize) =
      bincode::serde::decode_from_slice(bytes, config).map_err(|_| SerializationError::invalid_format())?;
    Ok(Box::new(payload))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for BincodeTelemetrySerializer {
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
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(BincodeTelemetrySerializer::new(serializer_id));

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
  let serializer_id = SerializerId::try_from(202).expect("valid serializer id");
  let setup = build_serialization_setup(serializer_id);
  let serialization_id = SerializationExtensionId::new(setup);

  let props = Props::from_fn(NullActor::new).with_name("serialization-bincode-demo");

  // デフォルト拡張が登録される前に独自シリアライザを差し替える
  let ext_id_clone = serialization_id.clone();
  let tick_driver = no_std_tick_driver_support::hardware_tick_driver_config();
  let config = ActorSystemConfig::default().with_tick_driver(tick_driver);
  let system = ActorSystem::new_with_config_and(&props, &config, move |system| {
    system.extended().register_extension(&ext_id_clone);
    Ok(())
  })
  .expect("actor system");

  let serialization: ArcShared<SerializationExtension> =
    system.extended().extension(&serialization_id).expect("extension registered");

  let payload = TelemetryPayload { node: 7, temperature: 24 };
  let serialized: SerializedMessage =
    serialization.serialize(&payload, SerializationCallScope::Remote).expect("serialize");

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
