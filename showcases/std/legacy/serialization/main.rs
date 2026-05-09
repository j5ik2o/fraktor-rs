//! Message serialization with serde_json and bincode.
//!
//! Demonstrates how to register custom `Serializer` / `SerializerWithStringManifest`
//! implementations so that messages can be encoded for remoting or persistence.
//!
//! - Part 1: JSON serialization (human-readable)
//! - Part 2: Bincode serialization (binary-efficient)
//!
//! Run with: `cargo run -p fraktor-showcases-std --example serialization`

use std::{
  any::{Any, TypeId},
  borrow::Cow,
};

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::{
  actor::{
    Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
    setup::ActorSystemConfig,
  },
  serialization::{
    NotSerializableError, SerializationCallScope, SerializationError, SerializationExtensionId,
    SerializationExtensionShared, SerializationSetup, SerializationSetupBuilder, SerializedMessage, Serializer,
    SerializerId, SerializerWithStringManifest, TransportInformation,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};
use serde::{Deserialize, Serialize};

// --- メッセージ定義 ---

const TELEMETRY_MANIFEST: &str = "sample.telemetry.TelemetryPayload";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct TelemetryPayload {
  node:        u16,
  temperature: i16,
}

// --- JSON シリアライザ ---

const JSON_SERIALIZER_NAME: &str = "telemetry-json";

struct JsonTelemetrySerializer {
  id: SerializerId,
}

impl JsonTelemetrySerializer {
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

impl Serializer for JsonTelemetrySerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<TelemetryPayload>().ok_or_else(|| self.payload_error())?;
    serde_json::to_vec(payload).map_err(|_| SerializationError::invalid_format())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let payload: TelemetryPayload = serde_json::from_slice(bytes).map_err(|_| SerializationError::invalid_format())?;
    Ok(Box::new(payload))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for JsonTelemetrySerializer {
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

// --- Bincode シリアライザ ---

const BINCODE_SERIALIZER_NAME: &str = "telemetry-bincode";

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
    let config = bincode::config::standard();
    bincode::serde::encode_to_vec(payload, config).map_err(|_| SerializationError::invalid_format())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let config = bincode::config::standard();
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

// --- ヘルパー ---

struct NullActor;

impl Actor for NullActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_serialization_setup(json_id: SerializerId, bincode_id: SerializerId) -> SerializationSetup {
  let json_serializer: ArcShared<dyn Serializer> = ArcShared::new(JsonTelemetrySerializer::new(json_id));
  let bincode_serializer: ArcShared<dyn Serializer> = ArcShared::new(BincodeTelemetrySerializer::new(bincode_id));

  SerializationSetupBuilder::new()
    // JSON シリアライザを登録
    .register_serializer(JSON_SERIALIZER_NAME, json_id, json_serializer)
    .expect("register json serializer")
    // Bincode シリアライザを登録
    .register_serializer(BINCODE_SERIALIZER_NAME, bincode_id, bincode_serializer)
    .expect("register bincode serializer")
    // デフォルトは JSON
    .set_fallback(JSON_SERIALIZER_NAME)
    .expect("set fallback")
    .bind::<TelemetryPayload>(JSON_SERIALIZER_NAME)
    .expect("bind type")
    .bind_remote_manifest::<TelemetryPayload>(TELEMETRY_MANIFEST)
    .expect("bind manifest")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("build setup")
}

fn serialize_and_restore(serialization: &SerializationExtensionShared, payload: &TelemetryPayload, label: &str) {
  let serialized: SerializedMessage =
    serialization.with_read(|ext| ext.serialize(payload, SerializationCallScope::Remote)).expect("serialize");
  println!("[{label}] manifest: {:?}", serialized.manifest());
  println!("[{label}] bytes ({} bytes): {:?}", serialized.bytes().len(), serialized.bytes());

  let decoded_any = serialization
    .with_read(|ext| ext.deserialize(&serialized, Some(TypeId::of::<TelemetryPayload>())))
    .expect("deserialize");
  let restored = *decoded_any.downcast::<TelemetryPayload>().expect("downcast payload");
  println!("[{label}] restored: node={}, temperature={}", restored.node, restored.temperature);
  assert_eq!(&restored, payload);
}

// --- エントリーポイント ---

fn main() {
  let json_id = SerializerId::try_from(201).expect("valid json serializer id");
  let bincode_id = SerializerId::try_from(202).expect("valid bincode serializer id");
  let setup = build_serialization_setup(json_id, bincode_id);
  let serialization_id = SerializationExtensionId::new(setup);

  let installers = ExtensionInstallers::default().with_extension_installer({
    let ext_id = serialization_id.clone();
    move |system: &fraktor_actor_core_rs::system::ActorSystem| {
      let registered = system.extended().register_extension(&ext_id);
      let existing = system.extended().extension(&ext_id).ok_or_else(|| {
        fraktor_actor_core_rs::system::ActorSystemBuildError::Configuration(
          "serialization extension was not retained".into(),
        )
      })?;
      if !fraktor_utils_core_rs::core::sync::ArcShared::ptr_eq(&registered, &existing) {
        return Err(fraktor_actor_core_rs::system::ActorSystemBuildError::Configuration(
          "serialization extension identity mismatch".into(),
        ));
      }
      Ok(())
    }
  });

  let props = Props::from_fn(|| NullActor).with_name("serialization-demo");
  let config = ActorSystemConfig::new(StdTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::create_from_props(&props, config).expect("actor system");

  let serialization: SerializationExtensionShared =
    (*system.extended().extension(&serialization_id).expect("extension registered")).clone();

  let payload = TelemetryPayload { node: 7, temperature: 24 };

  // Part 1: JSON シリアライゼーション（デフォルトバインド）
  println!("=== Part 1: JSON serialization ===");
  serialize_and_restore(&serialization, &payload, "json");

  // Part 2: Bincode シリアライゼーション（バインドを切り替え）
  println!("\n=== Part 2: Bincode serialization ===");
  serialization
    .with_write(|ext| ext.register_binding(TypeId::of::<TelemetryPayload>(), "TelemetryPayload", bincode_id))
    .expect("rebind to bincode");
  serialize_and_restore(&serialization, &payload, "bincode");

  // TransportInformation を使ったアクターパスのシリアライゼーション
  println!("\n=== Actor path serialization ===");
  let info = TransportInformation::new(Some(String::from("fraktor://sample@localhost:2552")));
  serialization.with_write(|ext| ext.push_transport_information(info));
  let path = serialization.with_read(|ext| ext.serialized_actor_path(&system.user_guardian_ref())).expect("actor path");
  println!("serialized actor path: {path}");
  let _ = serialization.with_write(|ext| ext.pop_transport_information());

  let termination = system.when_terminated();
  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
