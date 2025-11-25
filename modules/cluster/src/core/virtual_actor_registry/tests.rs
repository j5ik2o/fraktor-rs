use alloc::string::ToString;

use crate::core::{
  activation_error::ActivationError, grain_key::GrainKey, pid_cache_event::PidCacheEvent,
  virtual_actor_event::VirtualActorEvent, virtual_actor_registry::VirtualActorRegistry,
};

fn key(v: &str) -> GrainKey {
  GrainKey::new(v.to_string())
}

#[test]
fn same_key_returns_same_pid_until_owner_changes() {
  let mut registry = VirtualActorRegistry::new(8, 60);
  let authorities = vec!["a1:4000".to_string(), "a2:4001".to_string()];
  let k = key("user:1");

  let pid1 = registry.ensure_activation(&k, &authorities, 1, false, None).expect("activation");
  let pid2 = registry.ensure_activation(&k, &authorities, 2, false, None).expect("activation");

  assert_eq!(pid1, pid2);

  let owner = registry
    .drain_events()
    .into_iter()
    .find_map(|e| match e {
      | VirtualActorEvent::Activated { authority, .. } => Some(authority),
      | _ => None,
    })
    .expect("activated event present");

  // Hitイベン ト確認のため再度呼び出し。
  registry.ensure_activation(&k, &authorities, 2, false, None).expect("activation");

  registry.invalidate_authority(&owner);

  let _pid3 = registry.ensure_activation(&k, &["a2:4001".to_string()], 3, true, Some(vec![9])).expect("reactivation");

  let events = registry.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Activated { .. })));
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Hit { .. })));
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { .. })));
}

#[test]
fn passivates_when_idle_timeout_exceeded() {
  let mut registry = VirtualActorRegistry::new(4, 60);
  let k = key("user:2");
  registry.ensure_activation(&k, &["a1:4000".to_string()], 0, false, None).expect("activation");

  registry.passivate_idle(15, 10);

  let events = registry.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { .. })));
  assert!(registry.cached_pid(&k, 16).is_none());
}

#[test]
fn snapshot_missing_is_reported() {
  let mut registry = VirtualActorRegistry::new(2, 60);
  let k = key("user:3");
  let err = registry.ensure_activation(&k, &["a1:4000".to_string()], 0, true, None).expect_err("should fail");
  assert_eq!(err, ActivationError::SnapshotMissing { key: "user:3".to_string() });

  let events = registry.drain_events();
  assert!(events.iter().any(|e| matches!(e, VirtualActorEvent::SnapshotMissing { .. })));
}

// ============================================
// remove_activation テスト
// ============================================

#[test]
fn remove_activation_removes_activation_and_cache() {
  // アクティベーションを作成
  let mut registry = VirtualActorRegistry::new(8, 60);
  let k = key("user:remove_test");
  let authorities = ["a1:4000".to_string()];

  registry.ensure_activation(&k, &authorities, 0, false, None).expect("activation");
  registry.drain_events(); // Activated イベントをクリア

  // キャッシュが存在することを確認
  assert!(registry.cached_pid(&k, 1).is_some());

  // 削除実行
  registry.remove_activation(&k);

  // アクティベーションが削除されたことを確認（キャッシュも削除される）
  assert!(registry.cached_pid(&k, 2).is_none());

  // Passivated イベントが生成されたことを確認
  let events = registry.drain_events();
  assert!(
    events.iter().any(|e| matches!(e, VirtualActorEvent::Passivated { key } if *key == k)),
    "Passivated イベントが生成されるべき"
  );
}

#[test]
fn remove_activation_does_nothing_for_nonexistent_key() {
  // 空のレジストリを作成
  let mut registry = VirtualActorRegistry::new(8, 60);
  let k = key("user:nonexistent");

  // 存在しないキーに対して削除を実行（エラーなく完了すべき）
  registry.remove_activation(&k);

  // イベントは生成されないことを確認
  let events = registry.drain_events();
  assert!(events.is_empty(), "存在しないキーに対してはイベントが生成されるべきではない");
}

#[test]
fn remove_activation_allows_reactivation() {
  // アクティベーションを作成して削除し、再度アクティベーションできることを確認
  let mut registry = VirtualActorRegistry::new(8, 60);
  let k = key("user:reactivation_test");
  let authorities = ["a1:4000".to_string()];

  // 初回アクティベーション
  let pid1 = registry.ensure_activation(&k, &authorities, 0, false, None).expect("activation");
  registry.drain_events();

  // 削除
  registry.remove_activation(&k);
  registry.drain_events();

  // 再アクティベーション
  let pid2 = registry.ensure_activation(&k, &authorities, 1, false, None).expect("reactivation");

  // PID は同じ形式だが、Activated イベントが生成されるべき
  assert_eq!(pid1, pid2); // 同一 authority なので同じ PID

  let events = registry.drain_events();
  assert!(
    events.iter().any(|e| matches!(e, VirtualActorEvent::Activated { key, .. } if *key == k)),
    "削除後の再アクティベーションでは Activated イベントが生成されるべき"
  );
}

// ============================================
// drain_cache_events テスト
// ============================================

#[test]
fn drain_cache_events_returns_pid_cache_events() {
  // キャッシュイベントが生成される操作を実行し、drain_cache_events で取得できることを確認
  let mut registry = VirtualActorRegistry::new(8, 60);
  let k = key("user:cache_event_test");
  let authorities = ["a1:4000".to_string()];

  // アクティベーションを作成
  registry.ensure_activation(&k, &authorities, 0, false, None).expect("activation");

  // キャッシュを無効化してイベントを発生させる
  registry.invalidate_authority("a1:4000");

  // キャッシュイベントを取得
  let cache_events = registry.drain_cache_events();

  // PidCacheEvent::Dropped が生成されていることを確認
  assert!(
    cache_events.iter().any(|e| matches!(e, PidCacheEvent::Dropped { key, reason } if *key == k && reason == "quarantine")),
    "キャッシュ無効化時に Dropped イベントが生成されるべき"
  );
}

#[test]
fn drain_cache_events_clears_buffer() {
  // drain 後はイベントバッファがクリアされることを確認
  let mut registry = VirtualActorRegistry::new(8, 60);
  let k = key("user:cache_clear_test");
  let authorities = ["a1:4000".to_string()];

  // アクティベーションを作成してキャッシュイベントを発生させる
  registry.ensure_activation(&k, &authorities, 0, false, None).expect("activation");
  registry.invalidate_authority("a1:4000");

  // 最初の drain
  let first_drain = registry.drain_cache_events();
  assert!(!first_drain.is_empty(), "最初の drain ではイベントが存在するべき");

  // 2回目の drain
  let second_drain = registry.drain_cache_events();
  assert!(second_drain.is_empty(), "2回目の drain ではイベントバッファが空であるべき");
}

#[test]
fn drain_cache_events_returns_empty_when_no_events() {
  // イベントがない場合は空のベクターを返すことを確認
  let mut registry = VirtualActorRegistry::new(8, 60);

  let cache_events = registry.drain_cache_events();
  assert!(cache_events.is_empty(), "イベントがない場合は空のベクターを返すべき");
}

#[test]
fn drain_cache_events_captures_ttl_expiration() {
  // TTL 期限切れによるキャッシュイベントを確認
  let mut registry = VirtualActorRegistry::new(8, 10); // TTL 10秒
  let k = key("user:ttl_test");
  let authorities = ["a1:4000".to_string()];

  // アクティベーションを作成（now = 0）
  registry.ensure_activation(&k, &authorities, 0, false, None).expect("activation");

  // TTL 期限後にキャッシュを参照（now = 15 > expires_at = 10）
  let result = registry.cached_pid(&k, 15);
  assert!(result.is_none(), "TTL 期限切れでキャッシュミスになるべき");

  // キャッシュイベントを取得
  let cache_events = registry.drain_cache_events();
  assert!(
    cache_events.iter().any(|e| matches!(e, PidCacheEvent::Dropped { key, reason } if *key == k && reason.starts_with("expired_at_"))),
    "TTL 期限切れ時に Dropped イベントが生成されるべき"
  );
}
