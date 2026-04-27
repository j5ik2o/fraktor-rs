use alloc::string::String;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{IDENTIFY_MANIFEST, MiscMessageSerializer};
use crate::core::kernel::{
  actor::messaging::{AnyMessage, Identify},
  serialization::{
    builtin::{MISC_MESSAGE_ID, register_defaults},
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

fn serializer(registry: &ArcShared<SerializationRegistry>) -> MiscMessageSerializer {
  MiscMessageSerializer::new(MISC_MESSAGE_ID, registry.downgrade())
}

#[test]
fn identifier_returns_configured_id() {
  let registry = registry();
  assert_eq!(serializer(&registry).identifier(), MISC_MESSAGE_ID);
}

#[test]
fn include_manifest_is_true() {
  let registry = registry();
  assert!(serializer(&registry).include_manifest());
}

#[test]
fn manifest_for_identify_is_pekko_compatible_id() {
  let registry = registry();
  let s = serializer(&registry);
  let identify = Identify::new(AnyMessage::new(String::from("token")));
  let view = s.as_string_manifest().expect("string manifest view");
  assert_eq!(view.manifest(&identify), IDENTIFY_MANIFEST);
}

#[test]
fn identify_round_trips_with_string_correlation_id() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Identify::new(AnyMessage::new(String::from("correlation-42")));

  let bytes = s.to_binary(&original).expect("identify should encode");
  let decoded = s.from_binary(&bytes, None).expect("identify should decode");
  let identify = decoded.downcast::<Identify>().expect("decoded payload should be Identify");

  let restored = identify.correlation_id().downcast_ref::<String>().expect("correlation id should be String");
  assert_eq!(restored, "correlation-42");
}

#[test]
fn identify_round_trips_with_i32_correlation_id() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Identify::new(AnyMessage::new(7_i32));

  let bytes = s.to_binary(&original).expect("identify should encode");
  let decoded = s.from_binary(&bytes, None).expect("identify should decode");
  let identify = decoded.downcast::<Identify>().expect("decoded payload should be Identify");

  let restored = identify.correlation_id().downcast_ref::<i32>().expect("correlation id should be i32");
  assert_eq!(*restored, 7);
}

#[test]
fn from_binary_with_manifest_accepts_identify_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Identify::new(AnyMessage::new(String::from("payload")));

  let bytes = s.to_binary(&original).expect("identify should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded = view.from_binary_with_manifest(&bytes, IDENTIFY_MANIFEST).expect("manifest decode should succeed");
  let identify = decoded.downcast::<Identify>().expect("decoded payload should be Identify");
  let restored = identify.correlation_id().downcast_ref::<String>().expect("correlation id should be String");
  assert_eq!(restored, "payload");
}

#[test]
fn from_binary_with_unknown_manifest_returns_unknown_manifest_for_alias_fallback() {
  // 未対応 manifest は `UnknownManifest` を返さなければならない (`SerializationDelegator` の
  // manifest-route fallback がこの variant を見て次のシリアライザー候補へ continue する)。
  // `InvalidFormat` を返すと alias 経路が壊れる。
  let registry = registry();
  let s = serializer(&registry);
  let original = Identify::new(AnyMessage::new(String::from("payload")));
  let bytes = s.to_binary(&original).expect("identify should encode");

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, "AID");
  match result {
    | Err(SerializationError::UnknownManifest(manifest)) => assert_eq!(manifest, "AID"),
    | other => panic!("expected UnknownManifest(\"AID\"), got {other:?}"),
  }
}

#[test]
fn non_identify_message_type_is_rejected() {
  let registry = registry();
  let s = serializer(&registry);
  let result = s.to_binary(&123_i32);
  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}

#[test]
fn truncated_payload_is_rejected_on_decode() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Identify::new(AnyMessage::new(String::from("payload")));
  let mut bytes = s.to_binary(&original).expect("identify should encode");
  bytes.truncate(bytes.len() / 2);

  // 切り詰めバイト列は最終的に SerializedMessage::decode 経路で InvalidFormat に行き着く。
  // 単に is_err で受けず variant を固定して回帰検出感度を上げる。
  let result = s.from_binary(&bytes, None);
  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn registry_drop_yields_uninitialized_error_on_encode() {
  let registry = registry();
  let s = MiscMessageSerializer::new(MISC_MESSAGE_ID, registry.downgrade());
  drop(registry);

  let identify = Identify::new(AnyMessage::new(String::from("payload")));
  let result = s.to_binary(&identify);
  assert!(matches!(result, Err(SerializationError::Uninitialized)));
}
