use alloc::{boxed::Box, vec, vec::Vec};
use core::any::{Any, TypeId, type_name};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::serialization::{
  builder::SerializationSetupBuilder, builder_error::SerializationBuilderError, call_scope::SerializationCallScope,
  config_adapter::SerializationConfigAdapter, serializer::Serializer, serializer_id::SerializerId,
};

struct DummySerializer {
  id: SerializerId,
}

impl DummySerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for DummySerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    false
  }

  fn to_binary(
    &self,
    _message: &(dyn Any + Send + Sync),
  ) -> Result<Vec<u8>, crate::serialization::error::SerializationError> {
    Ok(Vec::new())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send>, crate::serialization::error::SerializationError> {
    Ok(Box::new(()))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

fn build_serializer(id: u32) -> (SerializerId, ArcShared<dyn Serializer>) {
  let serializer_id = SerializerId::try_from(id).expect("valid identifier");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(serializer_id));
  (serializer_id, serializer)
}

#[test]
fn build_without_fallback_fails() {
  match SerializationSetupBuilder::new().build() {
    | Err(error) => assert_eq!(error, SerializationBuilderError::MissingFallback),
    | Ok(_) => panic!("builder should require a fallback serializer"),
  }
}

#[test]
fn registers_serializer_and_binding() {
  let (serializer_id, serializer) = build_serializer(100);
  let builder = SerializationSetupBuilder::new()
    .register_serializer("dummy", serializer_id, serializer)
    .expect("register serializer");
  let builder = builder.set_fallback("dummy").expect("set fallback");
  let builder = builder.bind::<u32>("dummy").expect("bind type");
  let builder = builder.bind_remote_manifest::<u32>("example.Manifest").expect("bind manifest");
  let builder = builder.require_manifest_for_scope(SerializationCallScope::Remote);
  let setup = builder.build().expect("build succeeds");
  assert_eq!(setup.binding_for(TypeId::of::<u32>()), Some(serializer_id));
  assert!(setup.manifest_required_scopes().contains(&SerializationCallScope::Remote));
}

#[test]
fn binding_to_unknown_serializer_fails() {
  let builder = SerializationSetupBuilder::new();
  match builder.bind::<u8>("missing") {
    | Err(error) => assert_eq!(error, SerializationBuilderError::UnknownSerializer("missing".into())),
    | Ok(_) => panic!("binding should fail for unknown serializer"),
  }
}

#[test]
fn bind_remote_manifest_requires_binding() {
  let (serializer_id, serializer) = build_serializer(101);
  let builder = SerializationSetupBuilder::new()
    .register_serializer("dummy", serializer_id, serializer)
    .expect("register serializer")
    .set_fallback("dummy")
    .expect("fallback");
  match builder.bind_remote_manifest::<u32>("example.Manifest") {
    | Err(error) => assert_eq!(error, SerializationBuilderError::MarkerUnbound(type_name::<u32>().into())),
    | Ok(_) => panic!("manifest binding should fail without marker binding"),
  }
}

#[test]
fn bind_remote_manifest_records_manifest_string() {
  let (serializer_id, serializer) = build_serializer(150);
  let builder = SerializationSetupBuilder::new()
    .register_serializer("dummy", serializer_id, serializer)
    .expect("register serializer")
    .set_fallback("dummy")
    .expect("fallback")
    .bind::<u32>("dummy")
    .expect("bind type");
  let builder = builder.bind_remote_manifest::<u32>("example.Manifest").expect("bind manifest");
  let setup = builder.build().expect("build succeeds");
  assert_eq!(setup.manifest_for(TypeId::of::<u32>()), Some("example.Manifest"));
}

#[test]
fn manifest_routes_are_recorded() {
  let (serializer_id, serializer) = build_serializer(160);
  let builder = SerializationSetupBuilder::new()
    .register_serializer("dummy", serializer_id, serializer)
    .expect("register serializer")
    .set_fallback("dummy")
    .expect("fallback")
    .register_manifest_route("legacy.Manifest", 1, "dummy")
    .expect("register route");
  let setup = builder.build().expect("build succeeds");
  let routes = setup.manifest_routes();
  let route = routes.get("legacy.Manifest").expect("manifest entry");
  assert_eq!(route, &vec![(1, serializer_id)]);
}

#[test]
fn manifest_scope_requires_remote_manifest() {
  let (serializer_id, serializer) = build_serializer(170);
  let builder = SerializationSetupBuilder::new()
    .register_serializer("dummy", serializer_id, serializer)
    .expect("register")
    .set_fallback("dummy")
    .expect("fallback")
    .bind::<u32>("dummy")
    .expect("bind")
    .require_manifest_for_scope(SerializationCallScope::Remote);
  match builder.build() {
    | Err(SerializationBuilderError::ManifestRequired(SerializationCallScope::Remote)) => {},
    | Err(other) => panic!("unexpected error: {other:?}"),
    | Ok(_) => panic!("manifest validation should fail"),
  };
}

struct Adapter {
  id: u32,
}

impl SerializationConfigAdapter for Adapter {
  fn apply(&self, builder: SerializationSetupBuilder) -> Result<SerializationSetupBuilder, SerializationBuilderError> {
    let (serializer_id, serializer) = build_serializer(self.id);
    builder.register_serializer("adapter", serializer_id, serializer)
  }

  fn metadata(&self) -> &'static str {
    "test-adapter"
  }
}

#[test]
fn adapter_registration_is_applied_in_sequence() {
  let adapter = Adapter { id: 200 };
  let builder = SerializationSetupBuilder::new().apply_adapter(&adapter).expect("apply adapter");
  let builder = builder.set_fallback("adapter").expect("fallback");
  let setup = builder.build().expect("build succeeds");
  assert_eq!(setup.adapter_metadata(), ["test-adapter"]);
  assert!(setup.serializer(&SerializerId::try_from(200).expect("valid")).is_some());
}
