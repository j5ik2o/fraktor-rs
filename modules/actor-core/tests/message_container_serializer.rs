use core::{
  any::{Any, TypeId, type_name},
  convert::TryFrom,
};
use std::{borrow::Cow, boxed::Box, string::String, vec, vec::Vec};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    actor_selection::{ActorSelectionMessage, SelectionPathElement},
    messaging::AnyMessage,
  },
  serialization::{
    SerializationCallScope, SerializationError, SerializationSetupBuilder, Serializer, SerializerId,
    SerializerWithStringManifest,
    builtin::{MESSAGE_CONTAINER_ID, MessageContainerSerializer, STRING_ID, register_defaults},
    default_serialization_setup,
    serialization_registry::{SerializationRegistry, SerializerResolutionOrigin},
  },
};
use fraktor_utils_core_rs::core::sync::ArcShared;

#[derive(Debug, PartialEq, Eq)]
struct ManifestPayload(String);

struct ManifestPayloadSerializer {
  id: SerializerId,
}

impl ManifestPayloadSerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for ManifestPayloadSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<ManifestPayload>().ok_or(SerializationError::InvalidFormat)?;
    Ok(payload.0.as_bytes().to_vec())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Err(SerializationError::ManifestMissing { scope: SerializationCallScope::Remote })
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for ManifestPayloadSerializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed("manifest-payload-v1")
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    if manifest != "manifest-payload-v1" {
      return Err(SerializationError::UnknownManifest(String::from(manifest)));
    }
    let payload = core::str::from_utf8(bytes).map_err(|_| SerializationError::InvalidFormat)?;
    Ok(Box::new(ManifestPayload(String::from(payload))))
  }
}

fn default_registry() -> ArcShared<SerializationRegistry> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  register_defaults(&registry, |name, id| panic!("unexpected builtin collision: {name} {id:?}"))
    .expect("builtin defaults should register");
  registry
}

fn manifest_registry() -> (ArcShared<SerializationRegistry>, SerializerId) {
  let serializer_id = SerializerId::try_from(301).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(ManifestPayloadSerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("manifest_payload", serializer_id, serializer)
    .expect("register manifest serializer")
    .set_fallback("manifest_payload")
    .expect("set fallback")
    .bind::<ManifestPayload>("manifest_payload")
    .expect("bind payload")
    .build()
    .expect("build setup");
  (ArcShared::new(SerializationRegistry::from_setup(&setup)), serializer_id)
}

fn serializer(registry: &ArcShared<SerializationRegistry>) -> MessageContainerSerializer {
  MessageContainerSerializer::new(MESSAGE_CONTAINER_ID, registry.downgrade())
}

fn decode_selection(serializer: &MessageContainerSerializer, bytes: &[u8]) -> ActorSelectionMessage {
  let decoded = serializer.from_binary(bytes, None).expect("selection container should decode");
  *decoded.downcast::<ActorSelectionMessage>().expect("decoded payload should be ActorSelectionMessage")
}

fn round_trip(serializer: &MessageContainerSerializer, message: ActorSelectionMessage) -> ActorSelectionMessage {
  let bytes = serializer.to_binary(&message).expect("selection container should encode");
  decode_selection(serializer, &bytes)
}

fn selection_message(
  payload: AnyMessage,
  elements: Vec<SelectionPathElement>,
  wildcard_fan_out: bool,
) -> ActorSelectionMessage {
  ActorSelectionMessage::new(payload, elements, wildcard_fan_out)
}

#[test]
fn should_return_configured_serializer_id() {
  let registry = default_registry();
  let serializer = serializer(&registry);
  assert_eq!(serializer.identifier(), MESSAGE_CONTAINER_ID);
}

#[test]
fn should_not_require_manifest() {
  let registry = default_registry();
  let serializer = serializer(&registry);
  assert!(!serializer.include_manifest());
}

#[test]
fn should_round_trip_selection_path_elements_and_string_payload() {
  let registry = default_registry();
  let serializer = serializer(&registry);
  let elements = vec![
    SelectionPathElement::ChildName(String::from("worker")),
    SelectionPathElement::ChildPattern(String::from("task-*")),
    SelectionPathElement::Parent,
  ];
  let message = selection_message(AnyMessage::new(String::from("payload")), elements.clone(), false);

  let decoded = round_trip(&serializer, message);

  assert_eq!(decoded.elements(), elements.as_slice());
  assert!(!decoded.wildcard_fan_out());
  let payload = decoded.message().downcast_ref::<String>().expect("nested String payload");
  assert_eq!(payload, "payload");
}

#[test]
fn should_round_trip_wildcard_fan_out_flag() {
  let registry = default_registry();
  let serializer = serializer(&registry);
  let elements = vec![SelectionPathElement::ChildPattern(String::from("worker-*"))];
  let message = selection_message(AnyMessage::new(String::from("broadcast")), elements, true);

  let decoded = round_trip(&serializer, message);

  assert!(decoded.wildcard_fan_out());
  let payload = decoded.message().downcast_ref::<String>().expect("nested String payload");
  assert_eq!(payload, "broadcast");
}

#[test]
fn should_round_trip_nested_payload_with_string_manifest() {
  let (registry, serializer_id) = manifest_registry();
  let serializer = serializer(&registry);
  let message = selection_message(
    AnyMessage::new(ManifestPayload(String::from("manifested"))),
    vec![SelectionPathElement::ChildName(String::from("target"))],
    false,
  );

  let bytes = serializer.to_binary(&message).expect("selection container should encode");
  let serializer_id_bytes = serializer_id.value().to_le_bytes();
  assert!(
    bytes.windows(serializer_id_bytes.len()).any(|window| window == serializer_id_bytes),
    "nested serializer id must be present in the container"
  );
  assert!(
    bytes.windows(b"manifest-payload-v1".len()).any(|window| window == b"manifest-payload-v1"),
    "nested string manifest must be present in the container"
  );

  let decoded = decode_selection(&serializer, &bytes);
  let payload = decoded.message().downcast_ref::<ManifestPayload>().expect("manifest payload");
  assert_eq!(payload, &ManifestPayload(String::from("manifested")));
}

#[test]
fn should_reject_non_actor_selection_message_type() {
  let registry = default_registry();
  let serializer = serializer(&registry);
  let wrong_type = String::from("not a selection message");

  let Err(error) = serializer.to_binary(&wrong_type) else { panic!("wrong type should fail") };
  assert!(matches!(error, SerializationError::InvalidFormat));
}

#[test]
fn should_reject_unknown_nested_serializer_id() {
  let registry = default_registry();
  let serializer = serializer(&registry);
  let message = selection_message(
    AnyMessage::new(String::from("payload")),
    vec![SelectionPathElement::ChildName(String::from("worker"))],
    false,
  );
  let mut bytes = serializer.to_binary(&message).expect("selection container should encode");
  // 外側 wire レイアウト: [nested_len(4)] [nested_serializer_id(4)] [has_manifest(1)] ...
  // nested_serializer_id の位置は決定論的に offset 4 に固定なので、`STRING_ID` の偶発一致を
  // 待つバイト探索ではなく直接書き換える。
  let nested_id_offset = 4_usize;
  let unknown_id = SerializerId::try_from(9999).expect("unknown serializer id").value().to_le_bytes();
  // 比較として STRING_ID が確かに該当オフセットに格納されていることを表明し、レイアウト変更を
  // この箇所で検知できるようにする。
  assert_eq!(&bytes[nested_id_offset..nested_id_offset + 4], &STRING_ID.value().to_le_bytes());
  bytes[nested_id_offset..nested_id_offset + unknown_id.len()].copy_from_slice(&unknown_id);

  let Err(error) = serializer.from_binary(&bytes, None) else { panic!("unknown nested serializer should fail") };
  assert!(matches!(error, SerializationError::UnknownSerializer(_)));
}

#[test]
fn should_reject_truncated_selection_container() {
  let registry = default_registry();
  let serializer = serializer(&registry);
  let message = selection_message(
    AnyMessage::new(String::from("payload")),
    vec![SelectionPathElement::ChildName(String::from("worker"))],
    false,
  );
  let mut bytes = serializer.to_binary(&message).expect("selection container should encode");
  bytes.truncate(bytes.len() - 1);

  let Err(error) = serializer.from_binary(&bytes, None) else { panic!("truncated container should fail") };
  assert!(matches!(error, SerializationError::InvalidFormat));
}

#[test]
fn should_reject_unknown_selection_element_tag() {
  let registry = default_registry();
  let serializer = serializer(&registry);
  // 1 要素 (Parent) を含む有効な container を encode し、末尾の Parent タグバイトを未知の値に
  // 差し替えて unknown-tag の判定パスを実行させる。空 container (`&[u8::MAX]`) では nested 長
  // プレフィックス読み取り段階で先に失敗してしまい、目的のタグ判定パスを通らない。
  let message = selection_message(AnyMessage::new(String::from("payload")), vec![SelectionPathElement::Parent], false);
  let mut bytes = serializer.to_binary(&message).expect("selection container should encode");
  *bytes.last_mut().expect("Parent tag byte") = u8::MAX;

  let Err(error) = serializer.from_binary(&bytes, None) else { panic!("unknown element tag should fail") };
  assert!(matches!(error, SerializationError::InvalidFormat));
}

#[test]
fn should_register_actor_selection_message_in_builtin_defaults() {
  let registry = default_registry();

  let (resolved, origin) = registry
    .serializer_for_type(TypeId::of::<ActorSelectionMessage>(), type_name::<ActorSelectionMessage>(), None)
    .expect("actor selection message serializer should resolve");

  assert_eq!(resolved.identifier(), MESSAGE_CONTAINER_ID);
  assert_eq!(origin, SerializerResolutionOrigin::Binding);
  assert_eq!(registry.binding_name(TypeId::of::<ActorSelectionMessage>()).as_deref(), Some("ActorSelectionMessage"));
}
