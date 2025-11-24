#[path = "../std_tick_driver_support.rs"]
mod std_tick_driver_support;

use std::{
  any::{Any, TypeId},
  borrow::Cow,
  convert::TryFrom,
};

use fraktor_actor_rs::{
  core::{
    error::ActorError,
    extension::ExtensionInstallers,
    serialization::{
      NotSerializableError, SerializationCallScope, SerializationError, SerializationExtensionId, SerializationSetup,
      SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId, SerializerWithStringManifest,
      TransportInformation,
    },
    system::ActorSystemGeneric,
  },
  std::{
    actor_prim::{Actor, ActorContext},
    messaging::AnyMessageView,
    props::Props,
    system::{ActorSystem, ActorSystemConfig},
  },
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};
use serde::{Deserialize, Serialize};

const TELEMETRY_MANIFEST: &str = "sample.telemetry.TelemetryPayload";
const SERIALIZER_NAME: &str = "telemetry-json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TelemetryPayload {
  node:        u16,
  temperature: i16,
}

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

struct NullActor;

impl NullActor {
  fn new() -> Self {
    Self
  }
}

impl Actor for NullActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_, '_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_serialization_setup(serializer_id: SerializerId) -> SerializationSetup {
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(JsonTelemetrySerializer::new(serializer_id));

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

fn main() {
  let serializer_id = SerializerId::try_from(201).expect("valid serializer id");
  let setup = build_serialization_setup(serializer_id);
  let serialization_id = SerializationExtensionId::new(setup);
  let installers = ExtensionInstallers::default().with_extension_installer({
    let ext_id = serialization_id.clone();
    move |system: &ActorSystemGeneric<StdToolbox>| {
      system.extended().register_extension(&ext_id);
      Ok(())
    }
  });

  let props = Props::from_fn(NullActor::new).with_name("serialization-json-demo");
  let tick_driver = std_tick_driver_support::hardware_tick_driver_config();
  let config = ActorSystemConfig::default().with_tick_driver(tick_driver).with_extension_installers(installers);

  let system = ActorSystem::new_with_config(&props, &config).expect("actor system");
  let serialization: ArcShared<_> = system.extended().extension(&serialization_id).expect("extension registered");

  let payload = TelemetryPayload { node: 7, temperature: 24 };
  let serialized: SerializedMessage =
    serialization.serialize(&payload, SerializationCallScope::Remote).expect("serialize");

  println!("manifest: {:?}", serialized.manifest());
  println!("bytes: {:?}", serialized.bytes());

  let decoded_any =
    serialization.deserialize(&serialized, Some(TypeId::of::<TelemetryPayload>())).expect("deserialize");
  let restored = *decoded_any.downcast::<TelemetryPayload>().expect("downcast payload");
  println!("restored payload: node={}, temperature={}", restored.node, restored.temperature);

  let info = TransportInformation::new(Some(String::from("fraktor://sample@localhost:2552")));
  serialization.with_transport_information(info, || {
    let path = serialization.serialized_actor_path(&system.user_guardian_ref()).expect("actor path");
    println!("serialized actor path: {path}");
  });

  let termination = system.when_terminated();
  system.terminate().expect("terminate");
  while !termination.is_ready() {}
}
