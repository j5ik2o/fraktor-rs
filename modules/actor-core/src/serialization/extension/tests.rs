use alloc::{boxed::Box, vec, vec::Vec};
use core::any::{Any, TypeId};

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox,
  RuntimeToolbox,
  actor_prim::{
    Pid,
    actor_ref::{ActorRefGeneric, NullSender},
  },
  serialization::{
    call_scope::SerializationCallScope,
    error::SerializationError,
    extension::SerializationExtensionGeneric,
    serializer::Serializer,
    serializer_id::SerializerId,
    serialization_setup::SerializationSetup,
    transport_information::TransportInformation,
  },
  system::ActorSystemGeneric,
};

#[derive(Debug, PartialEq)]
struct TestPayload(u8);

struct TestSerializer {
  id: SerializerId,
}

impl TestSerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for TestSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    false
  }

  fn to_binary(&self, value: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, crate::serialization::error::SerializationError> {
    let payload = value.downcast_ref::<TestPayload>().expect("TestPayload expected");
    Ok(vec![payload.0])
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send>, crate::serialization::error::SerializationError> {
    Ok(Box::new(TestPayload(bytes.first().copied().unwrap_or_default())))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

fn build_extension<TB: RuntimeToolbox + 'static>(
  manifest: Option<&str>,
) -> (SerializationExtensionGeneric<TB>, SerializerId) {
  let serializer_id = SerializerId::try_from(300).expect("id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(TestSerializer::new(serializer_id));
  let mut builder = crate::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("test", serializer_id, serializer)
    .expect("register")
    .set_fallback("test")
    .expect("fallback")
    .bind::<TestPayload>("test")
    .expect("bind");
  if let Some(manifest) = manifest {
    builder = builder.bind_remote_manifest::<TestPayload>(manifest).expect("manifest");
    builder = builder.require_manifest_for_scope(SerializationCallScope::Remote);
  }
  let setup: SerializationSetup = builder.build().expect("build");
  let system = ActorSystemGeneric::<TB>::new_empty();
  (SerializationExtensionGeneric::new(&system, setup), serializer_id)
}

fn serialize_and_deserialize(extension: &SerializationExtensionGeneric<NoStdToolbox>) -> TestPayload {
  let payload = TestPayload(42);
  let serialized = extension
    .serialize(&payload, SerializationCallScope::Local)
    .expect("serialize");
  let any = extension.deserialize(&serialized, Some(TypeId::of::<TestPayload>())).expect("deserialize");
  *any.downcast::<TestPayload>().expect("downcast")
}

#[test]
fn serialize_local_round_trip() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  let result = serialize_and_deserialize(&extension);
  assert_eq!(result, TestPayload(42));
}

#[test]
fn serialize_remote_attaches_manifest() {
  let (extension, _) = build_extension::<NoStdToolbox>(Some("example.Manifest"));
  let payload = TestPayload(7);
  let serialized = extension.serialize(&payload, SerializationCallScope::Remote).expect("serialize");
  assert_eq!(serialized.manifest(), Some("example.Manifest"));
}

#[test]
fn with_transport_information_sets_scope_temporarily() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  assert!(extension.current_transport_information().is_none());
  let info = TransportInformation::new(Some("pekko://sys@host".into()));
  let value = extension.with_transport_information(info.clone(), || extension.current_transport_information());
  assert_eq!(value.as_ref(), Some(&info));
  assert!(extension.current_transport_information().is_none());
}

#[test]
fn serialized_actor_path_prefers_transport_address() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  let actor_ref = ActorRefGeneric::<NoStdToolbox>::new(Pid::new(1, 0), ArcShared::new(NullSender));
  let info = TransportInformation::new(Some("pekko://sys@host:2552".into()));
  let path = extension.with_transport_information(info, || extension.serialized_actor_path(&actor_ref)).expect("path");
  assert!(path.starts_with("pekko://sys@host:2552"));
}

#[test]
fn shutdown_rejects_future_serialization() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  extension.shutdown();
  let error = extension.serialize(&TestPayload(1), SerializationCallScope::Local).expect_err("should fail");
  assert!(matches!(error, SerializationError::Uninitialized));
}
