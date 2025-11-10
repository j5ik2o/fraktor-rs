use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

use cellactor_utils_core_rs::sync::ArcShared;

use crate::serialization::{
  builder::SerializationSetupBuilder,
  serialization_registry::SerializationRegistry,
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

  fn to_binary(&self, _message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, crate::serialization::error::SerializationError> {
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

#[test]
fn resolves_bound_serializer() {
  let (registry, serializer_id) = setup_with_binding();
  let resolved = registry.serializer_for_type(TypeId::of::<u32>());
  assert!(resolved.is_some(), "expected serializer to be resolved");
  let by_id = registry.serializer_by_id(serializer_id);
  assert!(by_id.is_some(), "serializer_by_id should match");
}

#[test]
fn returns_fallback_for_unknown_type() {
  let (registry, serializer_id) = setup_with_binding();
  let resolved = registry.serializer_for_type(TypeId::of::<u64>());
  let fallback = resolved.expect("fallback required");
  assert_eq!(fallback.identifier(), serializer_id);
}
