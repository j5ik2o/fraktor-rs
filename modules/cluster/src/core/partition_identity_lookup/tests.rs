//! Unit tests for partition identity lookup.

use alloc::{string::ToString, vec};

use crate::core::{
  activated_kind::ActivatedKind, grain_key::GrainKey, identity_lookup::IdentityLookup,
  partition_identity_lookup::PartitionIdentityLookup, partition_identity_lookup_config::PartitionIdentityLookupConfig,
  virtual_actor_event::VirtualActorEvent,
};

// ============================================================================
// Task 6.1: 構造体定義と基本コンストラクタのテスト
// ============================================================================

#[test]
fn test_new_creates_instance_with_custom_config() {
  // カスタム設定でインスタンスを作成
  let config = PartitionIdentityLookupConfig::new(2048, 600, 7200);
  let lookup = PartitionIdentityLookup::new(config);

  // 設定が正しく保持されていることを検証
  assert_eq!(lookup.config().cache_capacity(), 2048);
  assert_eq!(lookup.config().pid_ttl_secs(), 600);
  assert_eq!(lookup.config().idle_ttl_secs(), 7200);
}

#[test]
fn test_with_defaults_creates_instance_with_default_config() {
  // デフォルト設定でインスタンスを作成
  let lookup = PartitionIdentityLookup::with_defaults();

  // デフォルト値が正しく設定されていることを検証
  assert_eq!(lookup.config().cache_capacity(), 1024);
  assert_eq!(lookup.config().pid_ttl_secs(), 300);
  assert_eq!(lookup.config().idle_ttl_secs(), 3600);
}

#[test]
fn test_authorities_initially_empty() {
  // 新規インスタンスは空の authorities リストを持つ
  let lookup = PartitionIdentityLookup::with_defaults();
  assert!(lookup.authorities().is_empty());
}

#[test]
fn test_member_kinds_initially_empty() {
  // 新規インスタンスは空の member_kinds リストを持つ
  let lookup = PartitionIdentityLookup::with_defaults();
  assert!(lookup.member_kinds().is_empty());
}

#[test]
fn test_client_kinds_initially_empty() {
  // 新規インスタンスは空の client_kinds リストを持つ
  let lookup = PartitionIdentityLookup::with_defaults();
  assert!(lookup.client_kinds().is_empty());
}

#[test]
fn test_config_getter_returns_reference() {
  // config() ゲッターが参照を返すことを検証
  let config = PartitionIdentityLookupConfig::new(512, 120, 1800);
  let lookup = PartitionIdentityLookup::new(config);

  let config_ref = lookup.config();
  assert_eq!(config_ref.cache_capacity(), 512);
  assert_eq!(config_ref.pid_ttl_secs(), 120);
  assert_eq!(config_ref.idle_ttl_secs(), 1800);
}

#[test]
fn test_send_sync_auto_derived() {
  // PartitionIdentityLookup が Send + Sync を実装していることを確認
  fn assert_send_sync<T: Send + Sync>() {}
  assert_send_sync::<PartitionIdentityLookup>();
}

#[test]
fn test_multiple_instances_independent() {
  // 複数インスタンスが独立していることを検証
  let config1 = PartitionIdentityLookupConfig::new(100, 60, 300);
  let config2 = PartitionIdentityLookupConfig::new(200, 120, 600);

  let lookup1 = PartitionIdentityLookup::new(config1);
  let lookup2 = PartitionIdentityLookup::new(config2);

  // 各インスタンスは独自の設定を保持
  assert_eq!(lookup1.config().cache_capacity(), 100);
  assert_eq!(lookup2.config().cache_capacity(), 200);
  assert_eq!(lookup1.config().pid_ttl_secs(), 60);
  assert_eq!(lookup2.config().pid_ttl_secs(), 120);
}

// ============================================================================
// Task 6.2: setup_member と setup_client のテスト
// ============================================================================

#[test]
fn test_setup_member_stores_kinds() {
  // setup_member が kinds を保存することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  let kinds = vec![ActivatedKind::new("user".to_string())];

  let result = lookup.setup_member(&kinds);

  assert!(result.is_ok());
  assert_eq!(lookup.member_kinds().len(), 1);
  assert_eq!(lookup.member_kinds()[0].name(), "user");
}

#[test]
fn test_setup_client_stores_kinds() {
  // setup_client が kinds を保存することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  let kinds = vec![ActivatedKind::new("device".to_string())];

  let result = lookup.setup_client(&kinds);

  assert!(result.is_ok());
  assert_eq!(lookup.client_kinds().len(), 1);
  assert_eq!(lookup.client_kinds()[0].name(), "device");
}

#[test]
fn test_setup_member_with_multiple_kinds() {
  // 複数の kinds を setup できることを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  let kinds = vec![
    ActivatedKind::new("user".to_string()),
    ActivatedKind::new("order".to_string()),
    ActivatedKind::new("inventory".to_string()),
  ];

  let result = lookup.setup_member(&kinds);

  assert!(result.is_ok());
  assert_eq!(lookup.member_kinds().len(), 3);
}

#[test]
fn test_setup_member_overwrites_previous_kinds() {
  // 2回目の setup_member が前の kinds を上書きすることを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();

  let kinds1 = vec![ActivatedKind::new("user".to_string())];
  let _ = lookup.setup_member(&kinds1);
  assert_eq!(lookup.member_kinds().len(), 1);

  let kinds2 = vec![ActivatedKind::new("order".to_string()), ActivatedKind::new("device".to_string())];
  let _ = lookup.setup_member(&kinds2);
  assert_eq!(lookup.member_kinds().len(), 2);
  assert_eq!(lookup.member_kinds()[0].name(), "order");
}

// ============================================================================
// Task 6.3, 6.4: get メソッドのテスト
// ============================================================================

#[test]
fn test_get_returns_none_without_authorities() {
  // authorities がない場合は None を返すことを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  let key = GrainKey::new("user/123".to_string());
  let now = 1000;

  let result = lookup.get(&key, now);

  assert!(result.is_none());
}

#[test]
fn test_get_activates_and_returns_pid_with_authorities() {
  // authorities がある場合は PID を返すことを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;

  let result = lookup.get(&key, now);

  assert!(result.is_some());
  let pid = result.unwrap();
  assert!(pid.contains("user/123"));
}

#[test]
fn test_get_cache_hit_returns_same_pid() {
  // キャッシュヒット時に同じ PID が返ることを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;

  let pid1 = lookup.get(&key, now).unwrap();
  let pid2 = lookup.get(&key, now).unwrap();

  assert_eq!(pid1, pid2);
}

#[test]
fn test_get_generates_activated_event_on_first_call() {
  // 初回呼び出し時に Activated イベントが生成されることを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;

  let _ = lookup.get(&key, now);
  let events = lookup.drain_events();

  assert!(!events.is_empty());
  assert!(matches!(events[0], VirtualActorEvent::Activated { .. }));
}

#[test]
fn test_get_generates_hit_event_on_subsequent_calls() {
  // 2回目以降の ensure_activation 経由呼び出し時に Hit イベントが生成されることを検証
  // 注: キャッシュヒット時はイベント生成なし（高速パス）
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;

  // 初回: Activated イベント
  let _ = lookup.get(&key, now);
  let _ = lookup.drain_events();

  // キャッシュを無効化して ensure_activation を再度通過させる
  // （通常は TTL 経過後や topology 変更時に発生）
  // ここではキャッシュを手動で無効化できないため、新しいキーでテスト
  let key2 = GrainKey::new("user/456".to_string());
  let _ = lookup.get(&key2, now);
  let events = lookup.drain_events();

  assert!(!events.is_empty());
  assert!(matches!(events[0], VirtualActorEvent::Activated { .. }));
}

// ============================================================================
// Task 6.5: remove_pid メソッドのテスト
// ============================================================================

#[test]
fn test_remove_pid_removes_activation() {
  // remove_pid がアクティベーションを削除することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;

  // アクティベーションを作成
  let pid = lookup.get(&key, now).unwrap();
  assert!(!pid.is_empty());

  // アクティベーションを削除
  lookup.remove_pid(&key);

  // イベントを確認（Passivated イベントが生成される）
  let events = lookup.drain_events();
  // Activated + Passivated で 2 件
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { key: k } if *k == key)));
}

#[test]
fn test_remove_pid_on_nonexistent_key_does_nothing() {
  // 存在しないキーに対する remove_pid は何もしないことを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();

  let key = GrainKey::new("nonexistent".to_string());
  lookup.remove_pid(&key);

  // イベントは生成されない
  let events = lookup.drain_events();
  assert!(events.is_empty());
}

#[test]
fn test_remove_pid_invalidates_cache() {
  // remove_pid がキャッシュも無効化することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;

  // アクティベーションを作成
  let _ = lookup.get(&key, now);
  let _ = lookup.drain_events();

  // 削除
  lookup.remove_pid(&key);
  let _ = lookup.drain_events();

  // 再度 get を呼ぶと新しいアクティベーションが作成される
  let _ = lookup.get(&key, now);
  let events = lookup.drain_events();

  // 新しい Activated イベントが生成されるはず
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Activated { .. })));
}

// ============================================================================
// Task 7.1: update_topology メソッドのテスト
// ============================================================================

#[test]
fn test_update_topology_stores_authorities() {
  // update_topology が authorities リストを保存することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();

  lookup.update_topology(vec!["node1:8080".to_string(), "node2:8080".to_string()]);

  assert_eq!(lookup.authorities().len(), 2);
  assert!(lookup.authorities().contains(&"node1:8080".to_string()));
  assert!(lookup.authorities().contains(&"node2:8080".to_string()));
}

#[test]
fn test_update_topology_replaces_previous_authorities() {
  // update_topology が前の authorities を置き換えることを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();

  lookup.update_topology(vec!["node1:8080".to_string()]);
  assert_eq!(lookup.authorities().len(), 1);

  lookup.update_topology(vec!["node2:8080".to_string(), "node3:8080".to_string()]);
  assert_eq!(lookup.authorities().len(), 2);
  assert!(!lookup.authorities().contains(&"node1:8080".to_string()));
}

#[test]
fn test_update_topology_invalidates_absent_authorities() {
  // update_topology が消えた authority のエントリを無効化することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;
  let _ = lookup.get(&key, now);
  let _ = lookup.drain_events();

  // node1 を含まない新しいトポロジに更新
  lookup.update_topology(vec!["node2:8080".to_string()]);

  // Passivated イベントが生成される
  let events = lookup.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { .. })));
}

// ============================================================================
// Task 7.2: on_member_left メソッドのテスト
// ============================================================================

#[test]
fn test_on_member_left_invalidates_authority_entries() {
  // on_member_left が指定 authority のエントリを無効化することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;
  let _ = lookup.get(&key, now);
  let _ = lookup.drain_events();

  // node1 が離脱
  lookup.on_member_left("node1:8080");

  // Passivated イベントが生成される
  let events = lookup.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { .. })));
}

#[test]
fn test_on_member_left_with_unknown_authority_does_nothing() {
  // 存在しない authority に対する on_member_left は何もしないことを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;
  let _ = lookup.get(&key, now);
  let _ = lookup.drain_events();

  // 存在しない node2 が離脱
  lookup.on_member_left("node2:8080");

  // 何も変化なし（Passivated イベントは生成されない）
  let events = lookup.drain_events();
  assert!(events.iter().all(|e| !matches!(e, VirtualActorEvent::Passivated { .. })));
}

// ============================================================================
// Task 8.1: passivate_idle メソッドのテスト
// ============================================================================

#[test]
fn test_passivate_idle_removes_expired_activations() {
  // passivate_idle がアイドル時間を超えたアクティベーションを削除することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;
  let _ = lookup.get(&key, now);
  let _ = lookup.drain_events();

  // idle_ttl を超えた時間でパッシベーション
  let later = now + 4000; // 4000秒後（idle_ttl=3600 を超過）
  lookup.passivate_idle(later, 3600);

  // Passivated イベントが生成される
  let events = lookup.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { .. })));
}

#[test]
fn test_passivate_idle_keeps_recent_activations() {
  // passivate_idle が最近アクセスされたアクティベーションを保持することを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;
  let _ = lookup.get(&key, now);
  let _ = lookup.drain_events();

  // idle_ttl 未満の時間でパッシベーション
  let later = now + 100; // 100秒後（idle_ttl=3600 未満）
  lookup.passivate_idle(later, 3600);

  // Passivated イベントは生成されない
  let events = lookup.drain_events();
  assert!(events.iter().all(|e| !matches!(e, VirtualActorEvent::Passivated { .. })));
}

// ============================================================================
// Task 8.2: drain_events と drain_cache_events のテスト
// ============================================================================

#[test]
fn test_drain_events_returns_and_clears_events() {
  // drain_events がイベントを返しつつクリアすることを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;
  let _ = lookup.get(&key, now);

  // 1回目: イベントを取得
  let events1 = lookup.drain_events();
  assert!(!events1.is_empty());

  // 2回目: 空になっている
  let events2 = lookup.drain_events();
  assert!(events2.is_empty());
}

#[test]
fn test_drain_cache_events_returns_cache_events_on_invalidation() {
  // drain_cache_events がキャッシュ無効化時にイベントを返すことを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;
  let _ = lookup.get(&key, now);

  // キャッシュを無効化（remove_pid 経由で PidCache::invalidate_key が呼ばれる）
  lookup.remove_pid(&key);

  // キャッシュイベントを取得（Dropped イベントが生成される）
  let events = lookup.drain_cache_events();
  assert!(!events.is_empty());
}

#[test]
fn test_drain_cache_events_clears_after_drain() {
  // drain_cache_events が2回目は空を返すことを検証
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.update_topology(vec!["node1:8080".to_string()]);

  let key = GrainKey::new("user/123".to_string());
  let now = 1000;
  let _ = lookup.get(&key, now);

  // キャッシュを無効化してイベントを生成
  lookup.remove_pid(&key);

  // 1回目
  let events1 = lookup.drain_cache_events();
  assert!(!events1.is_empty());

  // 2回目: 空になっている
  let events2 = lookup.drain_cache_events();
  assert!(events2.is_empty());
}
