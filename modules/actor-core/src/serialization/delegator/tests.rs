use alloc::{borrow::Cow, boxed::Box, string::String, vec, vec::Vec};
use core::any::{Any, TypeId};

use ahash::RandomState;
use fraktor_utils_core_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::serialization::{
  builder::SerializationSetupBuilder, call_scope::SerializationCallScope, delegator::SerializationDelegator,
  error::SerializationError, serialization_registry::SerializationRegistry, serialization_setup::SerializationSetup,
  serialized_message::SerializedMessage, serializer::Serializer, serializer_id::SerializerId,
  string_manifest_serializer::SerializerWithStringManifest,
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

#[derive(Debug, PartialEq, Eq)]
struct ManifestPayload(&'static str);

struct PrimaryManifestSerializer {
  id: SerializerId,
}

impl Serializer for PrimaryManifestSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, _value: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    Ok(Vec::new())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    // 本テスト群は as_string_manifest 経路 (=from_binary_with_manifest) のみを使用するため、
    // ここは到達しない想定。万一呼ばれた場合はテスト前提が崩れたシグナルとして panic する。
    unreachable!("PrimaryManifestSerializer::from_binary should not be invoked in tests")
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for PrimaryManifestSerializer {
  fn manifest(&self, _value: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed("alias")
  }

  fn from_binary_with_manifest(
    &self,
    _bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Err(SerializationError::UnknownManifest(String::from(manifest)))
  }
}

struct AliasManifestSerializer {
  id: SerializerId,
}

impl Serializer for AliasManifestSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, _value: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    Ok(Vec::new())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(ManifestPayload("alias-hit")))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for AliasManifestSerializer {
  fn manifest(&self, _value: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed("alias")
  }

  fn from_binary_with_manifest(
    &self,
    _bytes: &[u8],
    _manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(ManifestPayload("alias-hit")))
  }
}

#[test]
fn delegator_deserializes_payload_via_registry() {
  // primary serializer を直接 hit させ、delegator.deserialize の happy path を通す。
  // alias 登録は本テストでは経由しないため意図的に省く (manifest-route の検証は
  // `delegator_routes_unknown_manifest_to_alias_serializer` で別途行う)。
  let primary_id = SerializerId::try_from(303).expect("id");
  let primary: ArcShared<dyn Serializer> = ArcShared::new(AliasManifestSerializer { id: primary_id });
  let setup = SerializationSetupBuilder::new()
    .register_serializer("primary", primary_id, primary)
    .expect("primary")
    .set_fallback("primary")
    .expect("fallback")
    .build()
    .expect("build");
  let registry = SerializationRegistry::from_setup(&setup);
  let delegator = SerializationDelegator::new(&registry);
  let message = SerializedMessage::new(primary_id, Some(String::from("alias")), Vec::new());
  let decoded = delegator.deserialize(&message, None).expect("decode");
  let payload = decoded.downcast::<ManifestPayload>().expect("ManifestPayload");
  assert_eq!(*payload, ManifestPayload("alias-hit"));
}

#[test]
fn delegator_routes_unknown_manifest_to_alias_serializer() {
  // primary が UnknownManifest を返す → manifest-route に登録した alias で復号できることを検証。
  let primary_id = SerializerId::try_from(305).expect("id");
  let alias_id = SerializerId::try_from(306).expect("id");
  let primary: ArcShared<dyn Serializer> = ArcShared::new(PrimaryManifestSerializer { id: primary_id });
  let alias: ArcShared<dyn Serializer> = ArcShared::new(AliasManifestSerializer { id: alias_id });
  let setup = SerializationSetupBuilder::new()
    .register_serializer("primary", primary_id, primary)
    .expect("primary")
    .register_serializer("alias", alias_id, alias)
    .expect("alias")
    .register_manifest_route("alias", 0, "alias")
    .expect("route")
    .set_fallback("primary")
    .expect("fallback")
    .build()
    .expect("build");
  let registry = SerializationRegistry::from_setup(&setup);
  let delegator = SerializationDelegator::new(&registry);
  let message = SerializedMessage::new(primary_id, Some(String::from("alias")), Vec::new());
  let decoded = delegator.deserialize(&message, None).expect("alias should be retried via manifest-route fallback");
  let payload = decoded.downcast::<ManifestPayload>().expect("ManifestPayload");
  assert_eq!(*payload, ManifestPayload("alias-hit"));
}

#[test]
fn delegator_propagates_unknown_manifest_when_no_alias_matches() {
  // manifest-route 未登録の場合、最終的に UnknownManifest がそのまま伝播する。
  let primary_id = SerializerId::try_from(307).expect("id");
  let primary: ArcShared<dyn Serializer> = ArcShared::new(PrimaryManifestSerializer { id: primary_id });
  let setup = SerializationSetupBuilder::new()
    .register_serializer("primary", primary_id, primary)
    .expect("primary")
    .set_fallback("primary")
    .expect("fallback")
    .build()
    .expect("build");
  let registry = SerializationRegistry::from_setup(&setup);
  let delegator = SerializationDelegator::new(&registry);
  let message = SerializedMessage::new(primary_id, Some(String::from("unknown-alias")), Vec::new());
  let error = delegator.deserialize(&message, None).expect_err("no alias registered");
  assert!(matches!(error, SerializationError::UnknownManifest(ref m) if m == "unknown-alias"), "got {error:?}");
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
