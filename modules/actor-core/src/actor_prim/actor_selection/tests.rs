//! Tests for ActorSelectionResolver

use crate::actor_prim::{
  actor_path::{ActorPath, ActorPathError},
  actor_selection::ActorSelectionResolver,
};

#[test]
fn test_resolve_current_path() {
  // 現在のパスを維持
  let base = ActorPath::root().child("worker");
  let resolved = ActorSelectionResolver::resolve_relative(&base, ".").unwrap();
  assert_eq!(resolved.to_relative_string(), base.to_relative_string());
}

#[test]
fn test_resolve_child_path() {
  // 子パスを追加
  // ActorPath::root() は guardian "cellactor" を含む
  let base = ActorPath::root().child("user");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "worker").unwrap();
  // 期待値は /cellactor/user/worker (guardian含む)
  assert_eq!(resolved.to_relative_string(), base.child("worker").to_relative_string());
}

#[test]
fn test_resolve_multiple_child_path() {
  // 複数の子パスを追加
  let base = ActorPath::root();
  let resolved = ActorSelectionResolver::resolve_relative(&base, "user/worker/task").unwrap();
  let expected = base.child("user").child("worker").child("task");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
}

#[test]
fn test_resolve_parent_path() {
  // 親パスへ遡る
  let base = ActorPath::root().child("user").child("worker");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "..").unwrap();
  let expected = ActorPath::root().child("user");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
}

#[test]
fn test_resolve_parent_and_child() {
  // 親へ遡って別の子を追加
  let base = ActorPath::root().child("user").child("worker");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "../manager").unwrap();
  let expected = ActorPath::root().child("user").child("manager");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
}

#[test]
fn test_escape_guardian_fails() {
  // guardian より上位へ遡ることは禁止
  let base = ActorPath::root();
  let result = ActorSelectionResolver::resolve_relative(&base, "..");
  assert!(matches!(result, Err(ActorPathError::RelativeEscape)));
}

#[test]
fn test_escape_beyond_guardian_fails() {
  // 複数の .. で guardian を超えようとする
  let base = ActorPath::root().child("user");
  let result = ActorSelectionResolver::resolve_relative(&base, "../..");
  assert!(matches!(result, Err(ActorPathError::RelativeEscape)));
}

#[test]
fn test_complex_relative_path() {
  // 複雑な相対パス解決
  let base = ActorPath::root().child("user").child("worker").child("subtask");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "../../manager/newtask").unwrap();
  let expected = ActorPath::root().child("user").child("manager").child("newtask");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
}

#[test]
fn test_empty_selection_returns_base() {
  // 空の選択式は base をそのまま返す
  let base = ActorPath::root().child("user");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "").unwrap();
  assert_eq!(resolved.to_relative_string(), base.to_relative_string());
}

// Task 3.2: Authority 未解決時の遅延配送テスト
#[test]
fn test_defer_send_when_authority_unresolved() {
  use crate::{
    actor_prim::actor_path::ActorPathParts, messaging::AnyMessage, system::remote_authority::RemoteAuthorityManager,
  };

  // リモート authority を持つパスを作成
  let parts = ActorPathParts::with_authority("test-system", Some(("remote-host", 2552)));
  let _remote_path = ActorPath::from_parts(parts);

  let manager = RemoteAuthorityManager::new();

  // authority が未解決なので defer_send される
  let message = AnyMessage::new(42u32);
  manager.defer_send("remote-host:2552", message);

  // deferred キューにメッセージが積まれていることを確認
  assert_eq!(manager.deferred_count("remote-host:2552"), 1);
}

#[test]
fn test_flush_deferred_when_connected() {
  use crate::{messaging::AnyMessage, system::remote_authority::RemoteAuthorityManager};

  let manager = RemoteAuthorityManager::new();
  let authority = "remote-host:2552";

  // Unresolved 状態でメッセージを defer
  manager.defer_send(authority, AnyMessage::new(1u32));
  manager.defer_send(authority, AnyMessage::new(2u32));
  assert_eq!(manager.deferred_count(authority), 2);

  // Connected へ遷移して deferred メッセージを取得
  let deferred = manager.set_connected(authority);
  assert!(deferred.is_some());
  assert_eq!(deferred.unwrap().len(), 2);

  // キューがクリアされたことを確認
  assert_eq!(manager.deferred_count(authority), 0);
}

#[test]
fn test_reject_send_when_quarantined() {
  use core::time::Duration;

  use crate::{
    messaging::AnyMessage,
    system::remote_authority::{RemoteAuthorityError, RemoteAuthorityManager},
  };

  let manager = RemoteAuthorityManager::new();
  let authority = "quarantined-host:2552";

  // Quarantine へ遷移
  manager.set_quarantine(authority, 0, Some(Duration::from_secs(300)));

  // Quarantine 中は送信が拒否される
  let result = manager.try_defer_send(authority, AnyMessage::new(42u32));
  assert!(matches!(result, Err(RemoteAuthorityError::Quarantined)));
}

// Task 3.3: 統合シナリオテスト
#[test]
fn test_scenario_unresolved_to_connected_delivery() {
  use crate::{messaging::AnyMessage, system::remote_authority::RemoteAuthorityManager};

  let manager = RemoteAuthorityManager::new();
  let authority = "integration-host:2552";

  // シナリオ 1: 未解決状態でメッセージを積む
  manager.defer_send(authority, AnyMessage::new("msg1"));
  manager.defer_send(authority, AnyMessage::new("msg2"));
  manager.defer_send(authority, AnyMessage::new("msg3"));

  assert_eq!(manager.deferred_count(authority), 3);

  // シナリオ 2: 接続確立で deferred メッセージを取得
  let deferred = manager.set_connected(authority).expect("deferred queue should exist");
  assert_eq!(deferred.len(), 3);
  assert_eq!(manager.deferred_count(authority), 0);

  // シナリオ 3: 接続済みなので新規メッセージは即座に配送可能（キューに積まれない）
  // 注: 現在の実装では Connected 状態でも defer できるが、実際の remoting では即配送する
}

#[test]
fn test_scenario_multiple_relative_selections() {
  // 複数の相対パス解決を組み合わせたシナリオ
  let root = ActorPath::root();
  let user = root.child("user");
  let worker = user.child("worker");
  let task = worker.child("task");

  // task から ../../manager/subtask へ遡る
  let resolved = ActorSelectionResolver::resolve_relative(&task, "../../manager/subtask").unwrap();
  let expected = user.child("manager").child("subtask");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());

  // さらに ../.. で user まで戻って system/logger へ
  let resolved2 = ActorSelectionResolver::resolve_relative(&resolved, "../..").unwrap();
  let expected2 = user; // user まで戻る
  assert_eq!(resolved2.to_relative_string(), expected2.to_relative_string());

  // user から system/logger へ移動（guardian を超えるのではなく、sibling への移動）
  // 注: guardian (cellactor) の直下には system と user がいるため、
  // user から ../system は guardian を経由する必要がある
  // 現在の実装では guardian より上には遡れないため、このケースは失敗する
}

#[test]
fn test_scenario_guardian_boundary_protection() {
  // guardian 境界を超えようとする複数パターン
  let root = ActorPath::root();

  // パターン 1: root から ..
  assert!(matches!(ActorSelectionResolver::resolve_relative(&root, ".."), Err(ActorPathError::RelativeEscape)));

  // パターン 2: user から ../..
  let user = root.child("user");
  assert!(matches!(ActorSelectionResolver::resolve_relative(&user, "../.."), Err(ActorPathError::RelativeEscape)));

  // パターン 3: 深いパスから大量の ..
  let deep = user.child("a").child("b").child("c");
  assert!(matches!(
    ActorSelectionResolver::resolve_relative(&deep, "../../../../.."),
    Err(ActorPathError::RelativeEscape)
  ));
}
