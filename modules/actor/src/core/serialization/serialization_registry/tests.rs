use alloc::{boxed::Box, vec, vec::Vec};
use core::any::{Any, TypeId, type_name};

use ahash::RandomState;
use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::core::serialization::{
  builder::SerializationSetupBuilder,
  error::SerializationError,
  serialization_registry::{SerializationRegistry, SerializerResolutionOrigin},
  serialization_setup::SerializationSetup,
  serializer::Serializer,
  serializer_id::SerializerId,
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
  ) -> Result<Vec<u8>, crate::core::serialization::error::SerializationError> {
    Ok(Vec::new())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, crate::core::serialization::error::SerializationError> {
    Ok(Box::new(()))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

fn setup_with_binding() -> (SerializationRegistry, SerializerId) {
  let serializer_id = SerializerId::try_from(120).expect("valid id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(serializer_id));
  let builder = SerializationSetupBuilder::new()
    .register_serializer("dummy", serializer_id, serializer)
    .expect("register serializer")
    .set_fallback("dummy")
    .expect("fallback")
    .bind::<u32>("dummy")
    .expect("bind type");
  let setup = builder.build().expect("build");
  (SerializationRegistry::from_setup(&setup), serializer_id)
}

fn setup_with_two_serializers() -> (SerializationRegistry, SerializerId, SerializerId) {
  let alpha_id = SerializerId::try_from(140).expect("alpha id");
  let beta_id = SerializerId::try_from(141).expect("beta id");
  let alpha: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(alpha_id));
  let beta: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(beta_id));
  let builder = SerializationSetupBuilder::new()
    .register_serializer("alpha", alpha_id, alpha)
    .expect("alpha")
    .register_serializer("beta", beta_id, beta)
    .expect("beta")
    .set_fallback("alpha")
    .expect("fallback")
    .bind::<u32>("alpha")
    .expect("bind");
  let setup = builder.build().expect("build");
  (SerializationRegistry::from_setup(&setup), alpha_id, beta_id)
}

#[test]
fn resolves_bound_serializer() {
  let (registry, serializer_id) = setup_with_binding();
  let (resolved, origin) =
    registry.serializer_for_type(TypeId::of::<u32>(), type_name::<u32>(), None).expect("resolved");
  assert_eq!(resolved.identifier(), serializer_id);
  assert_eq!(origin, SerializerResolutionOrigin::Binding);
  let by_id = registry.serializer_by_id(serializer_id).expect("serializer_by_id should match");
  assert_eq!(by_id.identifier(), serializer_id);
}

#[test]
fn returns_fallback_for_unknown_type() {
  let (registry, serializer_id) = setup_with_binding();
  let (fallback, origin) =
    registry.serializer_for_type(TypeId::of::<u64>(), type_name::<u64>(), None).expect("fallback required");
  assert_eq!(fallback.identifier(), serializer_id);
  assert_eq!(origin, SerializerResolutionOrigin::Fallback);
}

#[test]
fn serializer_by_id_unknown_returns_error() {
  let (registry, _, _) = setup_with_two_serializers();
  let unknown = SerializerId::try_from(199).expect("valid");
  let Err(error) = registry.serializer_by_id(unknown) else { panic!("expected unknown serializer error") };
  assert!(matches!(error, SerializationError::UnknownSerializer(id) if id == unknown));
}

#[test]
fn register_binding_allows_dynamic_resolution() {
  let (registry, alpha_id, beta_id) = setup_with_two_serializers();
  registry.register_binding(TypeId::of::<u64>(), type_name::<u64>(), beta_id).expect("register binding");
  let (resolved, origin) =
    registry.serializer_for_type(TypeId::of::<u64>(), type_name::<u64>(), None).expect("resolved");
  assert_eq!(resolved.identifier(), beta_id);
  assert_eq!(origin, SerializerResolutionOrigin::Binding);
  // second call should hit cache and still return beta
  let (cached, cache_origin) =
    registry.serializer_for_type(TypeId::of::<u64>(), type_name::<u64>(), None).expect("cached");
  assert_eq!(cached.identifier(), beta_id);
  assert_eq!(cache_origin, SerializerResolutionOrigin::Cache);
  // original binding remains
  let (original, original_origin) =
    registry.serializer_for_type(TypeId::of::<u32>(), type_name::<u32>(), None).expect("alpha");
  assert_eq!(original.identifier(), alpha_id);
  assert_eq!(original_origin, SerializerResolutionOrigin::Binding);
}

#[test]
fn manifest_routes_return_serializers_in_priority_order() {
  let alpha_id = SerializerId::try_from(150).expect("alpha");
  let beta_id = SerializerId::try_from(151).expect("beta");
  let alpha: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(alpha_id));
  let beta: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(beta_id));
  let builder = SerializationSetupBuilder::new()
    .register_serializer("alpha", alpha_id, alpha)
    .expect("alpha")
    .register_serializer("beta", beta_id, beta)
    .expect("beta")
    .register_manifest_route("legacy.Manifest", 2, "alpha")
    .expect("route")
    .register_manifest_route("legacy.Manifest", 1, "beta")
    .expect("route")
    .set_fallback("alpha")
    .expect("fallback");
  let setup = builder.build().expect("build");
  let registry = SerializationRegistry::from_setup(&setup);
  let list = registry.serializers_for_manifest("legacy.Manifest");
  let identifiers: Vec<SerializerId> = list.iter().map(|serializer| serializer.identifier()).collect();
  assert_eq!(identifiers, vec![beta_id, alpha_id], "priority order should be ascending");
}

#[test]
fn missing_serializer_produces_not_serializable_error() {
  let serializer_id = SerializerId::try_from(170).expect("id");
  let setup = SerializationSetup::testing_from_raw(
    HashMap::with_hasher(RandomState::new()),
    HashMap::with_hasher(RandomState::new()),
    HashMap::with_hasher(RandomState::new()),
    HashMap::with_hasher(RandomState::new()),
    HashMap::with_hasher(RandomState::new()),
    Vec::new(),
    serializer_id,
    Vec::new(),
  );
  let registry = SerializationRegistry::from_setup(&setup);
  let Err(error) = registry.serializer_for_type(TypeId::of::<u8>(), type_name::<u8>(), None) else {
    panic!("expected failure")
  };
  match error {
    | SerializationError::NotSerializable(payload) => assert_eq!(payload.type_name(), type_name::<u8>()),
    | other => panic!("unexpected error: {other:?}"),
  }
}
