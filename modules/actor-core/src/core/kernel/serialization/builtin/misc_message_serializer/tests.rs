use alloc::string::String;
use core::any::TypeId;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  ACTOR_IDENTITY_MANIFEST, IDENTIFY_MANIFEST, MiscMessageSerializer, REMOTE_ROUTER_CONFIG_MANIFEST,
  REMOTE_SCOPE_MANIFEST, STATUS_FAILURE_MANIFEST, STATUS_SUCCESS_MANIFEST,
};
use crate::core::kernel::{
  actor::{
    Address,
    deploy::RemoteScope,
    error::ActorError,
    messaging::{ActorIdentity, AnyMessage, Identify, Status},
  },
  routing::{ConsistentHashingPool, Pool, RemoteRouterConfig, RouterConfig, SmallestMailboxPool},
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
  let config = decoded
    .downcast::<RemoteRouterConfig<SmallestMailboxPool>>()
    .expect("decoded payload should be RemoteRouterConfig<SmallestMailboxPool>");

  assert_eq!(config.local().nr_of_instances(), 3);
  assert_eq!(config.local().router_dispatcher(), "remote-router-dispatcher");
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
      assert_eq!(error.type_name(), "RemoteRouterConfig<ConsistentHashingPool>");
      assert_eq!(error.serializer_id(), Some(MISC_MESSAGE_ID));
    },
    | other => panic!("expected NotSerializable for ConsistentHashingPool, got {other:?}"),
  }
}

#[test]
fn default_registry_does_not_bind_consistent_hashing_remote_router_config() {
  let registry = registry();
  let type_id = TypeId::of::<RemoteRouterConfig<ConsistentHashingPool>>();

  let binding_name = registry.binding_name(type_id);

  assert!(binding_name.is_none(), "ConsistentHashingPool binding must not select a lossy serializer");
}

#[test]
fn remote_scope_without_remote_authority_is_rejected() {
  let registry = registry();
  let s = serializer(&registry);
  let scope = RemoteScope::new(Address::local("local-sys"));

  let result = s.to_binary(&scope);

  assert!(matches!(result, Err(SerializationError::InvalidFormat)), "expected InvalidFormat, got {result:?}");
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
  let original = Status::failure(ActorError::recoverable_typed::<u32>("recoverable failure"));

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
fn registry_drop_yields_uninitialized_error_on_encode() {
  let registry = registry();
  let s = MiscMessageSerializer::new(MISC_MESSAGE_ID, registry.downgrade());
  drop(registry);

  let identify = Identify::new(AnyMessage::new(String::from("payload")));
  let result = s.to_binary(&identify);
  assert!(matches!(result, Err(SerializationError::Uninitialized)));
}
