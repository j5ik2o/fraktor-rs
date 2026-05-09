use alloc::{string::String, vec::Vec};
use core::any::TypeId;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  ACTOR_IDENTITY_MANIFEST, IDENTIFY_MANIFEST, MiscMessageSerializer, REMOTE_ROUTER_CONFIG_MANIFEST,
  REMOTE_SCOPE_MANIFEST, STATUS_FAILURE_MANIFEST, STATUS_SUCCESS_MANIFEST,
};
use crate::{
  actor::{
    Address,
    deploy::RemoteScope,
    error::{ActorError, ActorErrorReason},
    messaging::{ActorIdentity, AnyMessage, Identify, Status},
  },
  routing::{
    ConsistentHashingPool, RandomPool, RemoteRouterConfig, RemoteRouterPool, RoundRobinPool, SmallestMailboxPool,
  },
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

fn remote_node() -> Address {
  Address::remote("remote-sys", "10.0.0.1", 2552)
}

fn assert_serializable_pool_flags(pool: &RemoteRouterPool) {
  assert!(!pool.has_resizer());
  assert!(!pool.use_pool_dispatcher());
  assert!(pool.stop_router_when_all_routees_removed());
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
fn manifest_for_identify_is_pekko_compatible_a() {
  let registry = registry();
  let s = serializer(&registry);
  let identify = Identify::new(AnyMessage::new(String::from("token")));
  let view = s.as_string_manifest().expect("string manifest view");
  assert_eq!(view.manifest(&identify), IDENTIFY_MANIFEST);
  assert_eq!(view.manifest(&identify), "A");
}

#[test]
fn manifest_for_remote_scope_is_pekko_compatible_rs() {
  let registry = registry();
  let s = serializer(&registry);
  let scope = RemoteScope::new(remote_node());
  let view = s.as_string_manifest().expect("string manifest view");
  assert_eq!(view.manifest(&scope), REMOTE_SCOPE_MANIFEST);
}

#[test]
fn manifest_for_status_success_is_pekko_compatible_d() {
  let registry = registry();
  let s = serializer(&registry);
  let status = Status::success(AnyMessage::new(String::from("done")));
  let view = s.as_string_manifest().expect("string manifest view");
  assert_eq!(view.manifest(&status), STATUS_SUCCESS_MANIFEST);
}

#[test]
fn manifest_for_status_failure_is_pekko_compatible_e() {
  let registry = registry();
  let s = serializer(&registry);
  let status = Status::failure(ActorError::recoverable("failed"));
  let view = s.as_string_manifest().expect("string manifest view");
  assert_eq!(view.manifest(&status), STATUS_FAILURE_MANIFEST);
}

#[test]
fn manifest_for_actor_identity_is_pekko_compatible_b() {
  let registry = registry();
  let s = serializer(&registry);
  let identity = ActorIdentity::new(AnyMessage::new(String::from("correlation-actor")), None);
  let view = s.as_string_manifest().expect("string manifest view");
  assert_eq!(view.manifest(&identity), ACTOR_IDENTITY_MANIFEST);
}

#[test]
fn manifest_for_remote_router_config_is_pekko_compatible_rorrc() {
  let registry = registry();
  let s = serializer(&registry);
  let config = RemoteRouterConfig::new(SmallestMailboxPool::new(2), vec![remote_node()]);
  let view = s.as_string_manifest().expect("string manifest view");
  assert_eq!(view.manifest(&config), REMOTE_ROUTER_CONFIG_MANIFEST);
  assert_eq!(view.manifest(&config), "RORRC");
}

#[test]
fn manifest_for_round_robin_remote_router_config_is_pekko_compatible_rorrc() {
  let registry = registry();
  let s = serializer(&registry);
  let config = RemoteRouterConfig::new(RoundRobinPool::new(2), vec![remote_node()]);
  let view = s.as_string_manifest().expect("string manifest view");

  assert_eq!(view.manifest(&config), REMOTE_ROUTER_CONFIG_MANIFEST);
  assert_eq!(view.manifest(&config), "RORRC");
}

#[test]
fn manifest_for_random_remote_router_config_is_pekko_compatible_rorrc() {
  let registry = registry();
  let s = serializer(&registry);
  let config = RemoteRouterConfig::new(RandomPool::new(2), vec![remote_node()]);
  let view = s.as_string_manifest().expect("string manifest view");

  assert_eq!(view.manifest(&config), REMOTE_ROUTER_CONFIG_MANIFEST);
  assert_eq!(view.manifest(&config), "RORRC");
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
fn from_binary_with_manifest_accepts_legacy_identify_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Identify::new(AnyMessage::new(String::from("payload")));

  let bytes = s.to_binary(&original).expect("identify should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded = view.from_binary_with_manifest(&bytes, "ID").expect("legacy manifest decode should succeed");
  let identify = decoded.downcast::<Identify>().expect("decoded payload should be Identify");
  let restored = identify.correlation_id().downcast_ref::<String>().expect("correlation id should be String");
  assert_eq!(restored, "payload");
}

#[test]
fn actor_identity_without_ref_round_trips_correlation_id_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let original = ActorIdentity::new(AnyMessage::new(String::from("correlation-none")), None);

  let bytes = s.to_binary(&original).expect("actor identity should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded = view.from_binary_with_manifest(&bytes, ACTOR_IDENTITY_MANIFEST).expect("actor identity should decode");
  let identity = decoded.downcast::<ActorIdentity>().expect("decoded payload should be ActorIdentity");

  let restored = identity.correlation_id().downcast_ref::<String>().expect("correlation id should be String");
  assert_eq!(restored, "correlation-none");
  assert!(identity.actor_ref().is_none());
}

#[test]
fn remote_scope_round_trips_remote_address_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let node = remote_node();
  let original = RemoteScope::new(node.clone());

  let bytes = s.to_binary(&original).expect("remote scope should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded = view.from_binary_with_manifest(&bytes, REMOTE_SCOPE_MANIFEST).expect("remote scope should decode");
  let scope = decoded.downcast::<RemoteScope>().expect("decoded payload should be RemoteScope");

  assert_eq!(scope.node(), &node);
}

#[test]
fn remote_router_config_round_trips_smallest_mailbox_pool_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let first = remote_node();
  let second = Address::remote("remote-b", "10.0.0.2", 2553);
  let local = SmallestMailboxPool::new(3).with_dispatcher(String::from("remote-router-dispatcher"));
  let original = RemoteRouterConfig::new(local, vec![first.clone(), second.clone()]);

  let bytes = s.to_binary(&original).expect("remote router config should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded =
    view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST).expect("remote router config should decode");
  let config = decoded.downcast::<RemoteRouterConfig>().expect("decoded payload should be RemoteRouterConfig");

  assert_eq!(config.local().nr_of_instances(), 3);
  assert_eq!(config.local().router_dispatcher(), "remote-router-dispatcher");
  assert_serializable_pool_flags(config.local());
  assert!(matches!(config.local(), RemoteRouterPool::SmallestMailbox(_)));
  assert_eq!(config.nodes(), &[first, second]);
}

#[test]
fn remote_router_config_round_trips_round_robin_pool_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let first = remote_node();
  let second = Address::remote("remote-b", "10.0.0.2", 2553);
  let local = RoundRobinPool::new(3).with_dispatcher(String::from("round-robin-router-dispatcher"));
  let original = RemoteRouterConfig::new(local, vec![first.clone(), second.clone()]);

  let bytes = s.to_binary(&original).expect("remote router config should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded =
    view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST).expect("remote router config should decode");
  let config = decoded.downcast::<RemoteRouterConfig>().expect("decoded payload should be RemoteRouterConfig");

  assert_eq!(config.local().nr_of_instances(), 3);
  assert_eq!(config.local().router_dispatcher(), "round-robin-router-dispatcher");
  assert_serializable_pool_flags(config.local());
  assert!(matches!(config.local(), RemoteRouterPool::RoundRobin(_)));
  assert_eq!(config.nodes(), &[first, second]);
}

#[test]
fn remote_router_config_round_trips_random_pool_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let first = remote_node();
  let second = Address::remote("remote-b", "10.0.0.2", 2553);
  let local = RandomPool::new(3).with_dispatcher(String::from("random-router-dispatcher"));
  let original = RemoteRouterConfig::new(local, vec![first.clone(), second.clone()]);

  let bytes = s.to_binary(&original).expect("remote router config should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded =
    view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST).expect("remote router config should decode");
  let config = decoded.downcast::<RemoteRouterConfig>().expect("decoded payload should be RemoteRouterConfig");

  assert_eq!(config.local().nr_of_instances(), 3);
  assert_eq!(config.local().router_dispatcher(), "random-router-dispatcher");
  assert_serializable_pool_flags(config.local());
  assert!(matches!(config.local(), RemoteRouterPool::Random(_)));
  assert_eq!(config.nodes(), &[first, second]);
}

#[test]
fn remote_router_config_with_consistent_hashing_pool_is_rejected_without_lossy_mapper() {
  let registry = registry();
  let s = serializer(&registry);
  let first = remote_node();
  let second = Address::remote("remote-b", "10.0.0.2", 2553);
  let local = ConsistentHashingPool::new(4, |_message: &AnyMessage| 7)
    .with_dispatcher(String::from("consistent-router-dispatcher"));
  let original = RemoteRouterConfig::new(local, vec![first.clone(), second.clone()]);

  let result = s.to_binary(&original);

  match result {
    | Err(SerializationError::NotSerializable(error)) => {
      assert_eq!(error.type_name(), "ConsistentHashingPool");
      assert_eq!(error.serializer_id(), Some(MISC_MESSAGE_ID));
    },
    | other => panic!("expected NotSerializable for ConsistentHashingPool, got {other:?}"),
  }
}

#[test]
fn status_success_round_trips_string_payload_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Status::success(AnyMessage::new(String::from("ok")));

  let bytes = s.to_binary(&original).expect("status success should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded = view.from_binary_with_manifest(&bytes, STATUS_SUCCESS_MANIFEST).expect("status success should decode");
  let status = decoded.downcast::<Status>().expect("decoded payload should be Status");

  match *status {
    | Status::Success(payload) => {
      let restored = payload.downcast_ref::<String>().expect("success payload should be String");
      assert_eq!(restored, "ok");
    },
    | Status::Failure(_) => panic!("expected success"),
  }
}

#[test]
fn status_failure_round_trips_recoverable_reason_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Status::failure(ActorError::recoverable(ActorErrorReason::typed::<u32>("recoverable failure")));

  let bytes = s.to_binary(&original).expect("status failure should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded = view.from_binary_with_manifest(&bytes, STATUS_FAILURE_MANIFEST).expect("status failure should decode");
  let status = decoded.downcast::<Status>().expect("decoded payload should be Status");

  match *status {
    | Status::Failure(ActorError::Recoverable(reason)) => {
      assert_eq!(reason.as_str(), "recoverable failure");
      assert!(reason.source_type_id().is_none());
    },
    | other => panic!("expected recoverable failure, got {other:?}"),
  }
}

#[test]
fn status_failure_round_trips_fatal_reason_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Status::failure(ActorError::fatal("fatal failure"));

  let bytes = s.to_binary(&original).expect("status failure should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded = view.from_binary_with_manifest(&bytes, STATUS_FAILURE_MANIFEST).expect("status failure should decode");
  let status = decoded.downcast::<Status>().expect("decoded payload should be Status");

  match *status {
    | Status::Failure(ActorError::Fatal(reason)) => assert_eq!(reason.as_str(), "fatal failure"),
    | other => panic!("expected fatal failure, got {other:?}"),
  }
}

#[test]
fn status_failure_round_trips_escalate_reason_with_manifest() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Status::failure(ActorError::escalate("escalated failure"));

  let bytes = s.to_binary(&original).expect("status failure should encode");
  let view = s.as_string_manifest().expect("string manifest view");
  let decoded = view.from_binary_with_manifest(&bytes, STATUS_FAILURE_MANIFEST).expect("status failure should decode");
  let status = decoded.downcast::<Status>().expect("decoded payload should be Status");

  match *status {
    | Status::Failure(ActorError::Escalate(reason)) => assert_eq!(reason.as_str(), "escalated failure"),
    | other => panic!("expected escalated failure, got {other:?}"),
  }
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
fn actor_identity_decode_with_some_ref_returns_not_serializable_when_system_state_unavailable() {
  // Some(actor_ref) ペイロードを手で組み立てて、 deserialize_actor_ref の system_state 不在経路を
  // 直接踏ませる。 system_state を渡さない `MiscMessageSerializer::new` で構築した場合、
  // upgrade は常に None なので NotSerializable が返ることを確認する。
  let registry = registry();
  let s = serializer(&registry);
  // payload: 長さプリフィックス付き correlation_id (空 SerializedMessage), tag=1, path 文字列.
  let identify = Identify::new(AnyMessage::new(String::from("corr")));
  let identify_payload = s.to_binary(&identify).expect("encode identify");
  let mut bytes = Vec::new();
  bytes.extend_from_slice(&u32::try_from(identify_payload.len()).expect("len fits").to_le_bytes());
  bytes.extend_from_slice(&identify_payload);
  bytes.push(1);
  let path = "fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker";
  bytes.extend_from_slice(&u32::try_from(path.len()).expect("path len fits").to_le_bytes());
  bytes.extend_from_slice(path.as_bytes());

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, ACTOR_IDENTITY_MANIFEST);

  assert!(matches!(result, Err(SerializationError::NotSerializable(_))), "expected NotSerializable, got {result:?}");
}

#[test]
fn manifest_returns_empty_for_unsupported_type() {
  let registry = registry();
  let s = serializer(&registry);
  let view = s.as_string_manifest().expect("string manifest view");

  let manifest = view.manifest(&123_i32);

  assert!(manifest.is_empty(), "unsupported manifest lookup must fail safely");
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

#[test]
fn actor_identity_decode_rejects_truncated_actor_ref_tag() {
  let registry = registry();
  let s = serializer(&registry);
  let identity = ActorIdentity::new(AnyMessage::new(String::from("trunc")), None);
  let mut bytes = s.to_binary(&identity).expect("encode");
  // 末尾 1 バイト (actor_ref tag) を削り、 cursor.read_u8() が InvalidFormat になることを確認する。
  bytes.pop();

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, ACTOR_IDENTITY_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn remote_router_config_decode_rejects_truncated_dispatcher_string() {
  let registry = registry();
  let s = serializer(&registry);
  let mut bytes = Vec::new();
  bytes.push(1_u8);
  bytes.extend_from_slice(&3_u32.to_le_bytes());
  // dispatcher の長さフィールドだけ書き込んで本体を切り詰める。
  bytes.extend_from_slice(&5_u32.to_le_bytes());
  bytes.extend_from_slice(b"abc");

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn from_binary_uses_type_hint_to_select_decoder_for_identify() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Identify::new(AnyMessage::new(String::from("hint-id")));
  let bytes = s.to_binary(&original).expect("encode");

  let decoded = s.from_binary(&bytes, Some(TypeId::of::<Identify>())).expect("decode");

  let identify = decoded.downcast::<Identify>().expect("decoded payload should be Identify");
  let restored = identify.correlation_id().downcast_ref::<String>().expect("correlation id should be String");
  assert_eq!(restored, "hint-id");
}

#[test]
fn from_binary_uses_type_hint_to_select_decoder_for_remote_scope() {
  let registry = registry();
  let s = serializer(&registry);
  let original = RemoteScope::new(remote_node());
  let bytes = s.to_binary(&original).expect("encode");

  let decoded = s.from_binary(&bytes, Some(TypeId::of::<RemoteScope>())).expect("decode");

  let scope = decoded.downcast::<RemoteScope>().expect("decoded payload should be RemoteScope");
  assert_eq!(scope.node(), &remote_node());
}

#[test]
fn from_binary_uses_type_hint_to_select_decoder_for_remote_router_config() {
  let registry = registry();
  let s = serializer(&registry);
  let local = SmallestMailboxPool::new(2).with_dispatcher(String::from("d"));
  let original = RemoteRouterConfig::new(local, vec![remote_node()]);
  let bytes = s.to_binary(&original).expect("encode");

  let decoded = s.from_binary(&bytes, Some(TypeId::of::<RemoteRouterConfig>())).expect("decode");

  assert!(decoded.downcast_ref::<RemoteRouterConfig>().is_some());
}

#[test]
fn from_binary_uses_type_hint_to_select_decoder_for_round_robin_remote_router_config() {
  let registry = registry();
  let s = serializer(&registry);
  let local = RoundRobinPool::new(2).with_dispatcher(String::from("d"));
  let original = RemoteRouterConfig::new(local, vec![remote_node()]);
  let bytes = s.to_binary(&original).expect("encode");

  let decoded = s.from_binary(&bytes, Some(TypeId::of::<RemoteRouterConfig>())).expect("decode");

  assert!(decoded.downcast_ref::<RemoteRouterConfig>().is_some());
}

#[test]
fn from_binary_uses_type_hint_to_select_decoder_for_random_remote_router_config() {
  let registry = registry();
  let s = serializer(&registry);
  let local = RandomPool::new(2).with_dispatcher(String::from("d"));
  let original = RemoteRouterConfig::new(local, vec![remote_node()]);
  let bytes = s.to_binary(&original).expect("encode");

  let decoded = s.from_binary(&bytes, Some(TypeId::of::<RemoteRouterConfig>())).expect("decode");

  assert!(decoded.downcast_ref::<RemoteRouterConfig>().is_some());
}

#[test]
fn from_binary_returns_invalid_format_for_unsupported_type_hint() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Identify::new(AnyMessage::new(String::from("payload")));
  let bytes = s.to_binary(&original).expect("encode");

  let result = s.from_binary(&bytes, Some(TypeId::of::<i32>()));

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn from_binary_uses_type_hint_to_select_decoder_for_actor_identity() {
  let registry = registry();
  let s = serializer(&registry);
  let original = ActorIdentity::new(AnyMessage::new(String::from("hint")), None);
  let bytes = s.to_binary(&original).expect("encode");

  let decoded = s.from_binary(&bytes, Some(TypeId::of::<ActorIdentity>())).expect("decode");

  assert!(decoded.downcast_ref::<ActorIdentity>().is_some(), "decoded payload should be ActorIdentity");
}

#[test]
fn from_binary_rejects_status_without_manifest_routing() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Status::success(AnyMessage::new(String::from("ok")));
  let bytes = s.to_binary(&original).expect("encode");

  let result = s.from_binary(&bytes, Some(TypeId::of::<Status>()));

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn as_any_returns_concrete_serializer_reference() {
  let registry = registry();
  let s = serializer(&registry);

  let any_ref = s.as_any();

  assert!(any_ref.downcast_ref::<MiscMessageSerializer>().is_some());
}

#[test]
fn actor_identity_decode_rejects_unknown_actor_ref_tag() {
  let registry = registry();
  let s = serializer(&registry);
  let identity = ActorIdentity::new(AnyMessage::new(String::from("correlation")), None);
  let mut bytes = s.to_binary(&identity).expect("encode");
  // encode_actor_identity の wire 形式: [u32 LE: correlation_id len] [correlation_id bytes]
  // [u8: actor_ref tag] (Some の場合のみ以降に path)。
  // `bytes.len() - 1` で tag 位置を取ると、 wire 形式の末尾にフィールドを追加した瞬間に false
  // green になり得る。 length-prefix から actor_ref tag のオフセットを明示的に算出して、
  // tag バイトだけを変異させたケースで `InvalidFormat` を返すかを確認する。
  let correlation_len_prefix: [u8; 4] = bytes[..4].try_into().expect("correlation len prefix");
  let correlation_len = usize::try_from(u32::from_le_bytes(correlation_len_prefix)).expect("correlation len fits");
  let actor_ref_tag_index = 4 + correlation_len;
  assert_eq!(bytes[actor_ref_tag_index], 0, "encoded ActorIdentity::None must place tag=0 at the computed offset",);
  bytes[actor_ref_tag_index] = 9;

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, ACTOR_IDENTITY_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn actor_identity_decode_rejects_trailing_bytes_after_tag() {
  let registry = registry();
  let s = serializer(&registry);
  let identity = ActorIdentity::new(AnyMessage::new(String::from("correlation")), None);
  let mut bytes = s.to_binary(&identity).expect("encode");
  bytes.push(0xff);

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, ACTOR_IDENTITY_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn remote_scope_decode_rejects_trailing_bytes() {
  let registry = registry();
  let s = serializer(&registry);
  let scope = RemoteScope::new(remote_node());
  let mut bytes = s.to_binary(&scope).expect("encode");
  bytes.push(0xff);

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, REMOTE_SCOPE_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn remote_router_config_decode_rejects_zero_instances() {
  let registry = registry();
  let s = serializer(&registry);
  let mut bytes = Vec::new();
  bytes.push(1_u8);
  bytes.extend_from_slice(&0_u32.to_le_bytes());

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn remote_router_config_decode_rejects_node_count_exceeding_remaining_bytes() {
  let registry = registry();
  let s = serializer(&registry);
  let dispatcher = "remote-router-dispatcher";
  let mut bytes = Vec::new();
  bytes.push(1_u8);
  bytes.extend_from_slice(&3_u32.to_le_bytes());
  bytes.extend_from_slice(&u32::try_from(dispatcher.len()).expect("dispatcher fits in u32").to_le_bytes());
  bytes.extend_from_slice(dispatcher.as_bytes());
  // node_count を u32::MAX にして残りバイト数を遥かに超えさせる。 OOM 防御で InvalidFormat を返す。
  bytes.extend_from_slice(&u32::MAX.to_le_bytes());

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn remote_router_config_decode_rejects_node_count_exceeding_minimum_encoded_address_capacity() {
  let registry = registry();
  let s = serializer(&registry);
  let dispatcher = "remote-router-dispatcher";
  let mut bytes = Vec::new();
  bytes.push(1_u8);
  bytes.extend_from_slice(&3_u32.to_le_bytes());
  bytes.extend_from_slice(&u32::try_from(dispatcher.len()).expect("dispatcher fits in u32").to_le_bytes());
  bytes.extend_from_slice(dispatcher.as_bytes());
  bytes.extend_from_slice(&65_u32.to_le_bytes());
  bytes.extend(core::iter::repeat_n(0_u8, 1024));

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn remote_router_config_decode_rejects_zero_nodes() {
  let registry = registry();
  let s = serializer(&registry);
  let dispatcher = "remote-router-dispatcher";
  let mut bytes = Vec::new();
  bytes.push(1_u8);
  bytes.extend_from_slice(&3_u32.to_le_bytes());
  bytes.extend_from_slice(&u32::try_from(dispatcher.len()).expect("dispatcher fits in u32").to_le_bytes());
  bytes.extend_from_slice(dispatcher.as_bytes());
  bytes.extend_from_slice(&0_u32.to_le_bytes());

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn remote_router_config_decode_rejects_trailing_bytes() {
  let registry = registry();
  let s = serializer(&registry);
  let local = SmallestMailboxPool::new(2).with_dispatcher(String::from("d"));
  let original = RemoteRouterConfig::new(local, vec![remote_node()]);
  let mut bytes = s.to_binary(&original).expect("encode");
  bytes.push(0xff);

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn remote_router_config_decode_rejects_unknown_pool_tag() {
  let registry = registry();
  let s = serializer(&registry);
  let local = SmallestMailboxPool::new(2).with_dispatcher(String::from("d"));
  let original = RemoteRouterConfig::new(local, vec![remote_node()]);
  let mut bytes = s.to_binary(&original).expect("encode");
  bytes[0] = 0xff;

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, REMOTE_ROUTER_CONFIG_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn status_failure_decode_rejects_unknown_error_tag() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Status::failure(ActorError::recoverable("reason"));
  let mut bytes = s.to_binary(&original).expect("encode");
  bytes[0] = 0xff;

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, STATUS_FAILURE_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}

#[test]
fn status_failure_decode_rejects_trailing_bytes() {
  let registry = registry();
  let s = serializer(&registry);
  let original = Status::failure(ActorError::recoverable("reason"));
  let mut bytes = s.to_binary(&original).expect("encode");
  bytes.push(0xff);

  let view = s.as_string_manifest().expect("string manifest view");
  let result = view.from_binary_with_manifest(&bytes, STATUS_FAILURE_MANIFEST);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
}
