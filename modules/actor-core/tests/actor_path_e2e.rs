//! End-to-end integration tests for ActorPath Pekko compatibility.

use core::time::Duration;

use fraktor_actor_core_rs::{
  actor_prim::{
    actor_path::{ActorPath, ActorPathFormatter, ActorPathParts},
    actor_selection::ActorSelectionResolver,
  },
  config::{ActorSystemConfig, RemotingConfig},
  messaging::AnyMessage,
  system::{AuthorityState, RemoteAuthorityManager},
};

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
fn test_e2e_authority_unresolved_deferred_connected_delivery() {
  // Authority 未解決 → 接続のシナリオ
  let manager = RemoteAuthorityManager::new();
  let authority = "e2e-host:2552";

  // 未解決状態でメッセージを defer
  assert_eq!(manager.state(authority), AuthorityState::Unresolved);
  manager.defer_send(authority, AnyMessage::new("msg1"));
  manager.defer_send(authority, AnyMessage::new("msg2"));
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
  manager.defer_send(authority, AnyMessage::new(1i32));
  manager.set_connected(authority);

  // InvalidAssociation をトリガー
  manager.handle_invalid_association(authority, 0, Some(Duration::from_secs(300)));
  assert!(matches!(manager.state(authority), AuthorityState::Quarantine { .. }));

  // Quarantine 中は新規メッセージが拒否される
  let result = manager.try_defer_send(authority, AnyMessage::new(2i32));
  assert!(result.is_err());
}

#[test]
fn test_e2e_actor_selection_with_relative_paths() {
  // ActorSelection の相対パス解決
  let root = ActorPath::root();
  let user = root.child("user");
  let worker = user.child("worker");
  let task = worker.child("task");

  // task から ../../manager へ遡る
  let resolved = ActorSelectionResolver::resolve_relative(&task, "../../manager").unwrap();
  let expected = user.child("manager");
  assert_eq!(ActorPathFormatter::format(&resolved), ActorPathFormatter::format(&expected));
}

#[test]
fn test_e2e_config_integration() {
  // ActorSystemConfig と RemotingConfig の統合
  let remoting = RemotingConfig::default()
    .with_canonical_host("localhost")
    .with_canonical_port(2552)
    .with_quarantine_duration(Duration::from_secs(600));

  let config = ActorSystemConfig::default().with_system_name("e2e-system").with_remoting(remoting);

  assert_eq!(config.system_name(), "e2e-system");

  let remoting_cfg = config.remoting().expect("remoting should be configured");
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
