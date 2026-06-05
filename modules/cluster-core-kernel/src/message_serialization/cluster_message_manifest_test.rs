use alloc::{boxed::Box, vec, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::serialization::{
  SerializationError, SerializationExtension, SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId,
};
use fraktor_utils_core_rs::sync::ArcShared;

use super::ClusterMessageManifest;
use crate::message_serialization::ClusterMessagePayloadKind;

#[derive(Debug, PartialEq)]
struct TestPayload(u8);

struct VersionedSerializer {
  id: SerializerId,
}

impl VersionedSerializer {
  const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for VersionedSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, value: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = value.downcast_ref::<TestPayload>().expect("TestPayload expected");
    Ok(vec![payload.0])
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Err(SerializationError::UnknownManifest("missing.Manifest".into()))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

#[test]
fn manifest_preserves_actor_manifest_opaquely() {
  let manifest = ClusterMessageManifest::from_actor_manifest(Some("cluster.payload/gossip"));

  assert_eq!(manifest.actor_manifest(), Some("cluster.payload/gossip"));
}

#[test]
fn manifest_absence_is_preserved_without_payload_kind_fallback() {
  let kind = ClusterMessagePayloadKind::Gossip;
  let manifest = ClusterMessageManifest::from_actor_manifest(None);

  assert_eq!(kind.tag(), 1);
  assert_eq!(manifest.actor_manifest(), None);
}

#[test]
fn manifest_preservation_is_independent_from_payload_kind_tag() {
  let kind = ClusterMessagePayloadKind::Gossip;
  let manifest = ClusterMessageManifest::from_actor_manifest(Some("2"));

  assert_eq!(kind.tag(), 1);
  assert_eq!(manifest.actor_manifest(), Some("2"));
}

#[test]
fn unknown_actor_manifest_route_failure_remains_actor_core_deserialize_failure() {
  let serializer_id = SerializerId::try_from(422).expect("serializer");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(VersionedSerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("current", serializer_id, serializer)
    .expect("register")
    .set_fallback("current")
    .expect("fallback")
    .bind::<TestPayload>("current")
    .expect("bind")
    .build()
    .expect("build");
  let system = create_noop_actor_system();
  let extension = SerializationExtension::new(&system, setup);
  let manifest = ClusterMessageManifest::from_actor_manifest(Some("missing.Manifest"));
  let serialized = SerializedMessage::new(serializer_id, manifest.into_actor_manifest(), vec![99]);

  let error = extension.deserialize(&serialized, Some(TypeId::of::<TestPayload>())).expect_err("should fail");

  match error {
    | SerializationError::NotSerializable(payload) => {
      assert_eq!(payload.manifest(), Some("missing.Manifest"));
      assert_eq!(payload.serializer_id(), Some(serializer_id));
    },
    | other => panic!("unexpected error: {other:?}"),
  }
}
