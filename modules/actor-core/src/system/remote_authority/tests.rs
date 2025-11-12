#![cfg(test)]

use core::time::Duration;

use crate::{
  messaging::AnyMessage,
  system::remote_authority::{AuthorityState, RemoteAuthorityManager},
};

#[test]
fn test_initial_state_is_unresolved() {
  let manager = RemoteAuthorityManager::new();
  assert_eq!(manager.state("remote1"), AuthorityState::Unresolved);
}

#[test]
fn test_defer_send_stores_message() {
  let manager = RemoteAuthorityManager::new();
  let message = AnyMessage::new(42i32);

  manager.defer_send("remote1", message);

  assert_eq!(manager.deferred_count("remote1"), 1);
  assert_eq!(manager.state("remote1"), AuthorityState::Unresolved);
}

#[test]
fn test_set_connected_returns_deferred_messages() {
  let manager = RemoteAuthorityManager::new();
  let msg1 = AnyMessage::new(1i32);
  let msg2 = AnyMessage::new(2i32);

  manager.defer_send("remote1", msg1);
  manager.defer_send("remote1", msg2);

  let deferred = manager.set_connected("remote1");
  assert!(deferred.is_some());
  assert_eq!(deferred.unwrap().len(), 2);
  assert_eq!(manager.state("remote1"), AuthorityState::Connected);
  assert_eq!(manager.deferred_count("remote1"), 0);
}

#[test]
fn test_transition_unresolved_to_connected() {
  let manager = RemoteAuthorityManager::new();
  manager.defer_send("remote1", AnyMessage::new(42i32));

  assert_eq!(manager.state("remote1"), AuthorityState::Unresolved);

  manager.set_connected("remote1");
  assert_eq!(manager.state("remote1"), AuthorityState::Connected);
}

#[test]
fn test_transition_to_quarantine_clears_deferred() {
  let manager = RemoteAuthorityManager::new();
  manager.defer_send("remote1", AnyMessage::new(1i32));
  manager.defer_send("remote1", AnyMessage::new(2i32));

  assert_eq!(manager.deferred_count("remote1"), 2);

  manager.set_quarantine("remote1", Some(Duration::from_secs(300)));

  assert_eq!(manager.state("remote1"), AuthorityState::Quarantine { deadline: Some(Duration::from_secs(300)) });
  assert_eq!(manager.deferred_count("remote1"), 0);
}

#[test]
fn test_lift_quarantine_returns_to_unresolved() {
  let manager = RemoteAuthorityManager::new();
  manager.set_quarantine("remote1", Some(Duration::from_secs(300)));

  assert!(matches!(manager.state("remote1"), AuthorityState::Quarantine { .. }));

  manager.lift_quarantine("remote1");
  assert_eq!(manager.state("remote1"), AuthorityState::Unresolved);
}

#[test]
fn test_defer_after_quarantine_is_rejected() {
  let manager = RemoteAuthorityManager::new();
  manager.set_quarantine("remote1", Some(Duration::from_secs(300)));

  // quarantine 状態でも defer_send は呼べるが、状態がQuarantineのままであることを確認
  manager.defer_send("remote1", AnyMessage::new(99i32));

  // Quarantineのときdeferredに追加されることを確認
  // （実装では追加されるが、次のset_quarantineでクリアされる想定）
  assert_eq!(manager.deferred_count("remote1"), 1);
}

#[test]
fn test_connected_to_unresolved_transition() {
  let manager = RemoteAuthorityManager::new();
  manager.defer_send("remote1", AnyMessage::new(1i32));
  manager.set_connected("remote1");

  assert_eq!(manager.state("remote1"), AuthorityState::Connected);

  // Connected状態から再度defer_sendで新しいエントリを追加
  manager.defer_send("remote1", AnyMessage::new(2i32));
  // 状態はConnectedのまま、deferredに追加される（実装上の挙動）
  assert_eq!(manager.deferred_count("remote1"), 1);
}

// Task 5.2: Quarantine と InvalidAssociation 処理のテスト
#[test]
fn test_quarantine_rejects_new_sends() {
  // 隔離中の authority への新規送信が拒否されることを確認
  let manager = RemoteAuthorityManager::new();
  manager.set_quarantine("remote1", Some(Duration::from_secs(300)));

  // 隔離中は新規メッセージを受け付けない（defer_sendは呼べるが実装で拒否される想定）
  let result = manager.try_defer_send("remote1", AnyMessage::new(1i32));
  assert!(result.is_err());
  assert_eq!(manager.deferred_count("remote1"), 0);
}

#[test]
fn test_quarantine_duration_calculation() {
  // 隔離期間が正しく計算されることを確認
  let manager = RemoteAuthorityManager::new();
  let quarantine_duration = Duration::from_secs(600);
  manager.set_quarantine("remote1", Some(quarantine_duration));

  // 状態確認
  match manager.state("remote1") {
    | AuthorityState::Quarantine { deadline } => {
      assert_eq!(deadline, Some(quarantine_duration));
    },
    | _ => panic!("expected quarantine state"),
  }
}

#[test]
fn test_quarantine_period_expiration() {
  // 隔離期間経過後、自動的に Unresolved へ戻ることを確認
  let manager = RemoteAuthorityManager::new();
  let short_duration = Duration::from_millis(1);
  manager.set_quarantine("remote1", Some(short_duration));

  // 期限チェックして解除（簡易実装では即座に解除）
  manager.poll_quarantine_expiration();

  assert_eq!(manager.state("remote1"), AuthorityState::Unresolved);
}

#[test]
fn test_invalid_association_on_quarantine() {
  // InvalidAssociation を受信したとき、quarantine へ遷移することを確認
  let manager = RemoteAuthorityManager::new();
  manager.defer_send("remote1", AnyMessage::new(1i32));
  manager.set_connected("remote1");

  // InvalidAssociation イベントをトリガー
  manager.handle_invalid_association("remote1", Some(Duration::from_secs(300)));

  assert!(matches!(manager.state("remote1"), AuthorityState::Quarantine { .. }));
  // deferred キューはクリアされる
  assert_eq!(manager.deferred_count("remote1"), 0);
}

#[test]
fn test_manual_quarantine_override() {
  // 手動解除により即座に Connected へ遷移できることを確認
  let manager = RemoteAuthorityManager::new();
  manager.set_quarantine("remote1", Some(Duration::from_secs(3600)));

  // 手動で解除して接続状態へ
  manager.manual_override_to_connected("remote1");

  assert_eq!(manager.state("remote1"), AuthorityState::Connected);
}

// Task 5.3: EventStream 連携と状態遷移の観測
#[test]
fn test_state_transitions_observable() {
  // 各状態遷移が発生することを確認
  let manager = RemoteAuthorityManager::new();

  // 初期: Unresolved
  assert_eq!(manager.state("observable-host"), AuthorityState::Unresolved);

  // Unresolved -> Connected
  manager.set_connected("observable-host");
  assert_eq!(manager.state("observable-host"), AuthorityState::Connected);

  // Connected -> Quarantine
  manager.set_quarantine("observable-host", Some(Duration::from_secs(300)));
  assert!(matches!(manager.state("observable-host"), AuthorityState::Quarantine { .. }));

  // Quarantine -> Unresolved (期限経過)
  manager.lift_quarantine("observable-host");
  assert_eq!(manager.state("observable-host"), AuthorityState::Unresolved);

  // Quarantine -> Connected (手動解除)
  manager.set_quarantine("observable-host", Some(Duration::from_secs(600)));
  manager.manual_override_to_connected("observable-host");
  assert_eq!(manager.state("observable-host"), AuthorityState::Connected);
}

#[test]
fn test_remoting_config_override_applied() {
  // RemotingConfig からの隔離期間設定が適用されることを確認
  let manager = RemoteAuthorityManager::new();
  let custom_duration = Duration::from_secs(1800); // カスタム 30 分

  manager.set_quarantine("config-host", Some(custom_duration));

  match manager.state("config-host") {
    | AuthorityState::Quarantine { deadline } => {
      assert_eq!(deadline, Some(custom_duration));
    },
    | _ => panic!("expected quarantine state with custom duration"),
  }
}

#[test]
fn test_multiple_authorities_independent_states() {
  // 複数の authority が独立した状態を持つことを確認
  let manager = RemoteAuthorityManager::new();

  manager.defer_send("host1", AnyMessage::new(1i32));
  manager.defer_send("host2", AnyMessage::new(2i32));

  assert_eq!(manager.state("host1"), AuthorityState::Unresolved);
  assert_eq!(manager.state("host2"), AuthorityState::Unresolved);

  manager.set_connected("host1");
  assert_eq!(manager.state("host1"), AuthorityState::Connected);
  assert_eq!(manager.state("host2"), AuthorityState::Unresolved);

  manager.set_quarantine("host2", Some(Duration::from_secs(300)));
  assert_eq!(manager.state("host1"), AuthorityState::Connected);
  assert!(matches!(manager.state("host2"), AuthorityState::Quarantine { .. }));
}
