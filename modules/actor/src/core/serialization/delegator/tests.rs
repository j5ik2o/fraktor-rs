use alloc::{boxed::Box, vec, vec::Vec};
use core::any::{Any, TypeId};

use ahash::RandomState;
use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::core::serialization::{
  builder::SerializationSetupBuilder, call_scope::SerializationCallScope, delegator::SerializationDelegator,
  error::SerializationError, serialization_registry::SerializationRegistry, serialization_setup::SerializationSetup,
  serializer::Serializer, serializer_id::SerializerId,
};

#[derive(Clone)]
struct RecordingSerializer {
  id: SerializerId,
}

impl RecordingSerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for RecordingSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    false
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    if let Some(payload) = message.downcast_ref::<TestPayload>() {
      return Ok(payload.0.to_vec());
    }
    Ok(vec![0])
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

struct TestPayload(&'static [u8]);

#[test]
fn delegator_serializes_payload_via_registry() {
  let serializer_id = SerializerId::try_from(201).expect("id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(RecordingSerializer::new(serializer_id));
  let builder = SerializationSetupBuilder::new()
    .register_serializer("record", serializer_id, serializer)
    .expect("register")
    .set_fallback("record")
    .expect("fallback")
    .bind::<TestPayload>("record")
    .expect("bind");
  let setup = builder.build().expect("build");
  let registry = SerializationRegistry::from_setup(&setup);
  let delegator = SerializationDelegator::new(&registry);
  let payload = TestPayload(&[1, 2, 3]);
  let serialized = delegator.serialize(&payload, core::any::type_name::<TestPayload>()).expect("serialized");
  assert_eq!(serialized.serializer_id(), serializer_id);
  assert_eq!(serialized.bytes(), &[1, 2, 3]);
}

#[test]
fn delegator_propagates_not_serializable_errors() {
  let serializer_id = SerializerId::try_from(202).expect("id");
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
  let delegator = SerializationDelegator::new(&registry).with_scope(SerializationCallScope::Remote);
  let payload = TestPayload(&[9, 9, 9]);
  let error = delegator.serialize(&payload, core::any::type_name::<TestPayload>()).expect_err("missing binding");
  assert!(matches!(error, SerializationError::NotSerializable(_)));
}
