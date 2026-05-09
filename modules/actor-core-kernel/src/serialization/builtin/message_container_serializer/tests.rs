use alloc::{string::String, vec};

use fraktor_utils_core_rs::sync::ArcShared;

use super::MessageContainerSerializer;
use crate::{
  actor::{
    actor_selection::{ActorSelectionMessage, SelectionPathElement},
    messaging::AnyMessage,
  },
  serialization::{
    builtin::{MESSAGE_CONTAINER_ID, register_defaults},
    default_serialization_setup,
    error::SerializationError,
    serialization_registry::SerializationRegistry,
    serializer::Serializer,
  },
};

fn registry() -> ArcShared<SerializationRegistry> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  register_defaults(&registry, |_name, _id| {}).expect("builtins");
  registry
}

fn serializer(registry: &ArcShared<SerializationRegistry>) -> MessageContainerSerializer {
  MessageContainerSerializer::new(MESSAGE_CONTAINER_ID, registry.downgrade())
}

fn round_trip(message: ActorSelectionMessage) -> ActorSelectionMessage {
  let registry = registry();
  let serializer = serializer(&registry);
  let bytes = serializer.to_binary(&message).expect("selection message should encode");
  let decoded = serializer.from_binary(&bytes, None).expect("selection message should decode");
  *decoded.downcast::<ActorSelectionMessage>().expect("selection message")
}

// `identifier_returns_configured_id` / `include_manifest_is_false` /
// `non_selection_message_type_is_rejected` の各ケースは
// `modules/actor-core-kernel/tests/message_container_serializer.rs` の
// `should_return_configured_serializer_id` / `should_not_require_manifest` /
// `should_reject_non_actor_selection_message_type` と重複していたため、ここでは
// エンコーダ/デコーダの単体検証 (round-trip と element-tag 解析) のみを残す。

#[test]
fn actor_selection_message_round_trips_with_nested_string_payload() {
  let message = ActorSelectionMessage::new(
    AnyMessage::new(String::from("payload")),
    vec![SelectionPathElement::ChildName(String::from("worker")), SelectionPathElement::Parent],
    true,
  );

  let decoded = round_trip(message);

  assert_eq!(decoded.elements(), &[
    SelectionPathElement::ChildName(String::from("worker")),
    SelectionPathElement::Parent
  ]);
  assert!(decoded.wildcard_fan_out());
  assert_eq!(decoded.message().downcast_ref::<String>(), Some(&String::from("payload")));
}

#[test]
fn oversized_element_count_is_rejected_without_panicking() {
  // 信頼できない wire データから読み込んだ element_count をそのまま Vec::with_capacity に渡すと、
  // u32::MAX 級の値で multi-GB アロケート要求が発生し panic する。decode_selection が残バイト数を
  // 上限に capacity を制限することで silent panic にならないことを回帰検証する。
  let registry = registry();
  let serializer = serializer(&registry);
  // 0 要素で encode した payload (= 末尾 4 バイトが element_count) の element_count を
  // u32::MAX に書き換える。残バイト数 0 のため decode_selection は capacity 予約に進む前に
  // 「element_count > remaining」で InvalidFormat を返さなければならない。
  let nested = ActorSelectionMessage::new(AnyMessage::new(String::from("x")), Vec::new(), false);
  let mut bytes = serializer.to_binary(&nested).expect("nested encodes");
  let elem_count_offset = bytes.len() - 4;
  bytes[elem_count_offset..elem_count_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());

  let result = serializer.from_binary(&bytes, None);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn invalid_element_tag_is_rejected() {
  let registry = registry();
  let serializer = serializer(&registry);
  // 1 要素 (Parent) で encode し、末尾の Parent タグバイトを未知の値に差し替えて
  // unknown-tag 判定パスを実行させる。要素 0 で末尾に余剰バイトを付けると
  // is_finished の余剰バイト検知に先に当たり、目的のパスを通らない。
  let message =
    ActorSelectionMessage::new(AnyMessage::new(String::from("payload")), vec![SelectionPathElement::Parent], false);
  let mut bytes = serializer.to_binary(&message).expect("selection message should encode");
  *bytes.last_mut().expect("Parent tag byte") = u8::MAX;

  let result = serializer.from_binary(&bytes, None);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}
