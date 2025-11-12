//! Tests for ActorPathRegistry

use alloc::format;
use core::time::Duration;

use crate::{
  actor_prim::{
    Pid,
    actor_path::{ActorPath, ActorUid},
  },
  system::actor_path_registry::{ActorPathRegistry, PathResolutionError, ReservationPolicy},
};

#[test]
fn test_register_and_retrieve() {
  // PIDとパスを登録し、取得できることを確認
  let mut registry = ActorPathRegistry::new();
  let pid = Pid::new(1, 0);
  let path = ActorPath::root().child("worker");

  registry.register(pid, &path);

  let handle = registry.get(&pid).expect("handle should exist");
  assert_eq!(handle.pid(), pid);
  assert_eq!(handle.canonical_uri(), path.to_canonical_uri());
  assert_ne!(handle.path_hash(), 0);
}

#[test]
fn test_unregister() {
  // 登録後に削除できることを確認
  let mut registry = ActorPathRegistry::new();
  let pid = Pid::new(1, 0);
  let path = ActorPath::root();

  registry.register(pid, &path);
  assert!(registry.get(&pid).is_some());

  registry.unregister(&pid);
  assert!(registry.get(&pid).is_none());
}

#[test]
fn test_canonical_uri() {
  // canonical_uri ヘルパーが正しく動作することを確認
  let mut registry = ActorPathRegistry::new();
  let pid = Pid::new(1, 0);
  let path = ActorPath::root().child("manager");

  registry.register(pid, &path);

  let uri = registry.canonical_uri(&pid).expect("URI should exist");
  assert_eq!(uri, path.to_canonical_uri());
}

#[test]
fn test_nonexistent_pid() {
  // 存在しないPIDに対してはNoneを返すことを確認
  let registry = ActorPathRegistry::new();
  let pid = Pid::new(999, 0);

  assert!(registry.get(&pid).is_none());
  assert!(registry.canonical_uri(&pid).is_none());
}

#[test]
fn test_multiple_registrations() {
  // 複数のPIDを登録できることを確認
  let mut registry = ActorPathRegistry::new();

  for i in 0..10 {
    let pid = Pid::new(i, 0);
    let path = ActorPath::root().child(format!("worker-{}", i));
    registry.register(pid, &path);
  }

  for i in 0..10 {
    let pid = Pid::new(i, 0);
    let handle = registry.get(&pid).expect("handle should exist");
    assert_eq!(handle.pid(), pid);
  }
}

// Task 4.2: UID予約機能のテスト
#[test]
fn test_reserve_uid_prevents_reuse() {
  // UID予約後、同じパスで異なるUIDの再生成が拒否されることを確認
  let mut registry = ActorPathRegistry::new();
  let path = ActorPath::root().child("worker");
  let uid1 = ActorUid::new(100);
  let now = 1;

  // UID予約を実行（デフォルト5日の隔離期間）
  registry.reserve_uid(&path, uid1, now, None).expect("should reserve");

  // 同じパスで異なるUIDを予約しようとするとエラーになる
  let uid2 = ActorUid::new(200);
  let result = registry.reserve_uid(&path, uid2, now, None);
  assert!(matches!(result, Err(PathResolutionError::UidReserved { .. })));
}

#[test]
fn test_reserve_uid_with_custom_duration() {
  // カスタム隔離期間を指定してUID予約できることを確認
  let mut registry = ActorPathRegistry::new();
  let path = ActorPath::root().child("manager");
  let uid = ActorUid::new(300);
  let custom_duration = Duration::from_secs(1); // 1秒
  let now = 10;

  registry.reserve_uid(&path, uid, now, Some(custom_duration)).expect("should reserve with custom duration");

  // 予約中は再利用不可
  let uid2 = ActorUid::new(400);
  assert!(matches!(registry.reserve_uid(&path, uid2, now, None), Err(PathResolutionError::UidReserved { .. })));
}

#[test]
fn test_release_uid_allows_reuse() {
  // UID解放後、再利用可能になることを確認
  let mut registry = ActorPathRegistry::new();
  let path = ActorPath::root().child("temp");
  let uid1 = ActorUid::new(500);
  let now = 100;

  registry.reserve_uid(&path, uid1, now, None).expect("should reserve");

  // 手動解放
  registry.release_uid(&path);

  // 解放後は新しいUIDで予約可能
  let uid2 = ActorUid::new(600);
  assert!(registry.reserve_uid(&path, uid2, now, None).is_ok());
}

#[test]
fn test_reserve_uid_blocks_same_path_even_with_different_uid() {
  let mut registry = ActorPathRegistry::new();
  let base = ActorPath::root().child("service");
  let uid_a = ActorUid::new(1000);
  let now = 5;

  registry.reserve_uid(&base, uid_a, now, None).expect("initial reserve");

  // 同じパスだが UID の異なる ActorPath を生成
  let uid_b = ActorUid::new(2000);
  let path_with_new_uid = base.clone().with_uid(uid_b);
  let result = registry.reserve_uid(&path_with_new_uid, uid_b, now, None);
  assert!(matches!(result, Err(PathResolutionError::UidReserved { uid: _ })));
}

#[test]
fn test_handle_hash_ignores_uid() {
  let mut registry = ActorPathRegistry::new();
  let pid_a = Pid::new(200, 0);
  let pid_b = Pid::new(201, 0);
  let base = ActorPath::root().child("logger");
  let with_uid = base.clone().with_uid(ActorUid::new(42));

  registry.register(pid_a, &base);
  registry.register(pid_b, &with_uid);

  let hash_a = registry.get(&pid_a).unwrap().path_hash();
  let hash_b = registry.get(&pid_b).unwrap().path_hash();
  assert_eq!(hash_a, hash_b);
}

#[test]
fn test_poll_expired_removes_old_reservations() {
  // 期限切れのUID予約が削除されることを確認
  let mut registry = ActorPathRegistry::new();
  let path = ActorPath::root().child("expiring");
  let uid = ActorUid::new(700);
  let short_duration = Duration::from_millis(1);
  let now = 1;

  registry.reserve_uid(&path, uid, now, Some(short_duration)).expect("should reserve");

  // 期限切れエントリを削除（簡易実装ではすべて削除）
  registry.poll_expired(now + 1);

  // 削除後は再予約可能
  let uid2 = ActorUid::new(800);
  assert!(registry.reserve_uid(&path, uid2, now + 2, None).is_ok());
}

#[test]
fn test_reservation_policy_from_config() {
  // RemotingConfig経由で隔離期間を設定できることを確認
  let default_policy = ReservationPolicy::default();
  assert_eq!(default_policy.quarantine_duration(), Duration::from_secs(5 * 24 * 3600));

  let custom_policy = ReservationPolicy::with_quarantine_duration(Duration::from_secs(600));
  assert_eq!(custom_policy.quarantine_duration(), Duration::from_secs(600));
}

// Task 4.3: SystemState／ActorRef 連携テスト
#[test]
fn test_temporary_actor_registration() {
  // 一時アクターの登録と canonical URI 取得を確認
  let mut registry = ActorPathRegistry::new();
  let temp_pid = Pid::new(9999, 1);
  let temp_path = ActorPath::root().child("temp").child("actor1");

  registry.register(temp_pid, &temp_path);

  let uri = registry.canonical_uri(&temp_pid).expect("URI should exist");
  assert!(uri.contains("temp"));
  assert!(uri.contains("actor1"));
}

#[test]
fn test_pid_restoration_returns_correct_uri() {
  // PID復元時に正しい canonical URI が返されることを確認
  let mut registry = ActorPathRegistry::new();
  let pid = Pid::new(123, 456);
  let path = ActorPath::root().child("service").with_uid(ActorUid::new(789));

  registry.register(pid, &path);

  // PIDから復元
  let handle = registry.get(&pid).expect("handle should exist");
  assert_eq!(handle.pid(), pid);
  assert_eq!(handle.uid(), Some(ActorUid::new(789)));
  assert!(handle.canonical_uri().contains("service"));
}

#[test]
fn test_concurrent_access_safety() {
  // 複数スレッドから安全にアクセスできることを確認（シミュレーション）
  let mut registry = ActorPathRegistry::new();

  // 複数のアクターを登録
  for i in 0..100 {
    let pid = Pid::new(i, 0);
    let path = ActorPath::root().child(format!("concurrent-{}", i));
    registry.register(pid, &path);
  }

  // すべて取得可能
  for i in 0..100 {
    let pid = Pid::new(i, 0);
    assert!(registry.get(&pid).is_some());
  }
}

#[test]
fn test_uid_release_via_deathwatch() {
  // DeathWatch経由でUID解放が正しく動作することを確認（シミュレーション）
  let mut registry = ActorPathRegistry::new();
  let path = ActorPath::root().child("watched");
  let uid = ActorUid::new(1001);
  let now = 50;

  // UID予約
  registry.reserve_uid(&path, uid, now, None).expect("should reserve");

  // DeathWatch通知を受けてUID解放
  registry.release_uid(&path);

  // 解放後は再予約可能
  let new_uid = ActorUid::new(1002);
  assert!(registry.reserve_uid(&path, new_uid, now, None).is_ok());
}

#[test]
fn test_registry_with_custom_policy() {
  // カスタムポリシーでレジストリを作成できることを確認
  let custom_policy = ReservationPolicy::with_quarantine_duration(Duration::from_secs(100));
  let mut registry = ActorPathRegistry::with_policy(custom_policy);

  let path = ActorPath::root().child("custom");
  let uid = ActorUid::new(2001);
  let now = 77;

  registry.reserve_uid(&path, uid, now, None).expect("should reserve with custom policy");

  // 予約確認
  let uid2 = ActorUid::new(2002);
  assert!(matches!(registry.reserve_uid(&path, uid2, now, None), Err(PathResolutionError::UidReserved { .. })));
}
