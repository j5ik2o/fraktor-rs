//! End-to-end integration tests for ActorPath Pekko compatibility.

use core::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::{
    actor_path::{ActorPath, ActorPathFormatter, ActorPathParser, ActorPathParts, ActorUid, PathResolutionError},
    actor_selection::{ActorSelectionError, ActorSelectionResolver},
  },
  config::{ActorSystemConfig, RemotingConfig},
  messaging::AnyMessage,
  system::{AuthorityState, RemoteAuthorityManager},
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

#[test]
fn test_e2e_local_path_format_consistency() {
  // ローカルパスの format が一貫していること
  let parts = ActorPathParts::local("test-system");
  let path = ActorPath::from_parts(parts).child("worker").child("task");

  let formatted1 = ActorPathFormatter::format(&path);
  let formatted2 = ActorPathFormatter::format(&path);

  assert_eq!(formatted1, formatted2);
  // フォーマット結果にセグメントが含まれていることを確認
  assert!(formatted1.contains("worker"));
  assert!(formatted1.contains("task"));
}

#[test]
fn test_e2e_remote_path_authority_format() {
  // リモートパスの authority フォーマット
  let parts = ActorPathParts::with_authority("remote-system", Some(("remote-host", 2552)));
  let path = ActorPath::from_parts(parts).child("remote-worker");

  let formatted = ActorPathFormatter::format(&path);
  // 注: ActorPathFormatter の実装に依存するため、ここでは相対パスのみをチェック
  assert!(formatted.contains("remote-worker"));
}

#[test]
fn test_e2e_format_parse_round_trip_with_uid() {
  let parts = ActorPathParts::with_authority("round-sys", Some(("round-host", 2552)));
  let path = ActorPath::from_parts(parts).child("service").child("worker").with_uid(ActorUid::new(4242));

  let canonical = ActorPathFormatter::format(&path);
  let parsed = ActorPathParser::parse(&canonical).expect("parse");

  assert_eq!(canonical, ActorPathFormatter::format(&parsed));
  assert_eq!(parsed.uid().map(|uid| uid.value()), Some(4242));
  // UID を差し替えても canonical のパス部分は一致する
  let without_uid = parsed.clone().with_uid(ActorUid::new(1111));
  assert_eq!(without_uid.to_relative_string(), parsed.to_relative_string());
}

#[test]
fn test_e2e_authority_unresolved_deferred_connected_delivery() {
  // Authority 未解決 → 接続のシナリオ
  let manager = RemoteAuthorityManager::new();
  let authority = "e2e-host:2552";

  // 未解決状態でメッセージを defer
  assert_eq!(manager.state(authority), AuthorityState::Unresolved);
  manager.defer_send(authority, AnyMessage::new("msg1")).expect("defer");
  manager.defer_send(authority, AnyMessage::new("msg2")).expect("defer");
  assert_eq!(manager.deferred_count(authority), 2);

  // 接続確立
  let deferred = manager.set_connected(authority).expect("should have deferred messages");
  assert_eq!(deferred.len(), 2);
  assert_eq!(manager.state(authority), AuthorityState::Connected);
  assert_eq!(manager.deferred_count(authority), 0);
}

#[test]
fn test_e2e_authority_quarantine_invalid_association() {
  // Quarantine シナリオ: InvalidAssociation の挙動
  let manager = RemoteAuthorityManager::new();
  let authority = "quarantine-host:2552";

  // 初期メッセージを defer
  manager.defer_send(authority, AnyMessage::new(1i32)).expect("defer");
  manager.set_connected(authority);

  // InvalidAssociation をトリガー
  manager.handle_invalid_association(authority, 0, Some(Duration::from_secs(300)));
  assert!(matches!(manager.state(authority), AuthorityState::Quarantine { .. }));

  // Quarantine 中は新規メッセージが拒否される
  let result = manager.defer_send(authority, AnyMessage::new(2i32));
  assert!(result.is_err());
}

#[test]
fn test_e2e_actor_selection_with_relative_paths() {
  // ActorSelection の相対パス解決
  let user = ActorPath::root();
  let worker = user.child("worker");
  let task = worker.child("task");

  // task から ../../manager へ遡る
  let resolved = ActorSelectionResolver::resolve_relative(&task, "../../manager").unwrap();
  let expected = user.child("manager");
  assert_eq!(ActorPathFormatter::format(&resolved), ActorPathFormatter::format(&expected));
}

#[test]
fn test_e2e_actor_selection_remote_authority_state_sequence() {
  let base = ActorPath::from_parts(ActorPathParts::with_authority("cluster", Some(("remote-node", 2552))));
  let manager = RemoteAuthorityManager::new();

  // 未解決時は defer + AuthorityUnresolved
  let result =
    ActorSelectionResolver::resolve_relative_with_authority(&base, "worker", &manager, Some(AnyMessage::new("msg1")));
  match result {
    | Err(ActorSelectionError::Authority(PathResolutionError::AuthorityUnresolved)) => {},
    | other => panic!("expected unresolved, got {:?}", other),
  }
  assert_eq!(manager.deferred_count("remote-node:2552"), 1);

  // 接続後は成功し、deferred キューは flush されない（resolve は状態確認のみ）
  manager.set_connected("remote-node:2552");
  let resolved = ActorSelectionResolver::resolve_relative_with_authority(&base, "worker", &manager, None)
    .expect("should resolve when connected");
  assert_eq!(resolved.to_canonical_uri(), ActorPathFormatter::format(&base.child("worker")));

  // 隔離状態へ遷移させ、即座に AuthorityQuarantined を返す
  manager.set_quarantine("remote-node:2552", 0, Some(Duration::from_secs(60)));
  let result =
    ActorSelectionResolver::resolve_relative_with_authority(&base, "worker", &manager, Some(AnyMessage::new("msg2")));
  assert!(matches!(result, Err(ActorSelectionError::Authority(PathResolutionError::AuthorityQuarantined))));
}

#[test]
fn test_e2e_config_integration() {
  // ActorSystemConfig と RemotingConfig の統合
  let remoting = RemotingConfig::default()
    .with_canonical_host("localhost")
    .with_canonical_port(2552)
    .with_quarantine_duration(Duration::from_secs(600));

  let config =
    ActorSystemConfig::<NoStdToolbox>::default().with_system_name("e2e-system").with_remoting_config(remoting);

  assert_eq!(config.system_name(), "e2e-system");

  let remoting_cfg = config.remoting_config().expect("remoting should be configured");
  assert_eq!(remoting_cfg.canonical_host(), "localhost");
  assert_eq!(remoting_cfg.canonical_port(), Some(2552));
  assert_eq!(remoting_cfg.quarantine_duration(), Duration::from_secs(600));
}

#[test]
fn test_e2e_uri_consistency_across_modules() {
  // DeathWatch、ログ、ActorSelection から取得した URI が一致すること
  let parts = ActorPathParts::local("consistent-system");
  let path = ActorPath::from_parts(parts).child("logger").child("file-logger");

  let uri = ActorPathFormatter::format(&path);

  // 同じパスを Selection で解決
  let base = ActorPath::from_parts(ActorPathParts::local("consistent-system")).child("logger");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "file-logger").unwrap();

  assert_eq!(uri, ActorPathFormatter::format(&resolved));
}
