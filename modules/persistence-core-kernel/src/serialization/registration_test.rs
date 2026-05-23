use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationError, SerializationSetupBuilder, Serializer, SerializerId,
  serialization_registry::SerializationRegistry,
};
use fraktor_utils_core_rs::sync::ArcShared;

use super::{MESSAGE_SERIALIZER_ID, register_serializer};

struct DummySerializer {
  id: SerializerId,
}

impl DummySerializer {
  const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for DummySerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, _message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    Ok(Vec::new())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(()))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

fn registry_with_serializer(slot_id: SerializerId, serializer_id: SerializerId) -> SerializationRegistry {
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("dummy", slot_id, serializer)
    .expect("register")
    .set_fallback("dummy")
    .expect("fallback")
    .build()
    .expect("setup");
  SerializationRegistry::from_setup(&setup)
}

#[test]
fn register_serializer_accepts_existing_same_registration() {
  let registry = registry_with_serializer(MESSAGE_SERIALIZER_ID, MESSAGE_SERIALIZER_ID);
  let serializer: ArcShared<dyn Serializer> =
    ArcShared::new(DummySerializer::new(SerializerId::try_from(101).expect("serializer id")));

  let result = register_serializer(&registry, MESSAGE_SERIALIZER_ID, serializer, |existing| {
    existing.identifier() == MESSAGE_SERIALIZER_ID
  });

  assert!(result.is_ok());
  let existing = registry.registered_serializer(MESSAGE_SERIALIZER_ID).expect("serializer");
  assert_eq!(existing.identifier(), MESSAGE_SERIALIZER_ID);
  assert_eq!(existing.to_binary(&()).expect("binary"), Vec::<u8>::new());
  assert!(existing.from_binary(&[], None).expect("value").downcast_ref::<()>().is_some());
  assert!(existing.as_any().downcast_ref::<DummySerializer>().is_some());
}

#[test]
fn register_serializer_rejects_existing_conflicting_registration() {
  let wrong_id = SerializerId::try_from(101).expect("serializer id");
  let registry = registry_with_serializer(MESSAGE_SERIALIZER_ID, wrong_id);
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(DummySerializer::new(MESSAGE_SERIALIZER_ID));

  let result = register_serializer(&registry, MESSAGE_SERIALIZER_ID, serializer, |existing| {
    existing.identifier() == MESSAGE_SERIALIZER_ID
  });

  assert!(matches!(result, Err(SerializationError::SerializerIdCollision(id)) if id == MESSAGE_SERIALIZER_ID));
}
