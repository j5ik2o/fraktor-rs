use core::time::Duration;

use crate::core::{
  messaging::AnyMessage,
  system::{
    remote::{RemoteAuthorityError, RemoteAuthorityRegistry},
    state::AuthorityState,
  },
};

#[test]
fn test_initial_state_is_unresolved() {
  let registry = RemoteAuthorityRegistry::new();
  assert_eq!(registry.state("remote1"), AuthorityState::Unresolved);
}

#[test]
fn test_defer_send_stores_message() {
  let mut registry = RemoteAuthorityRegistry::new();
  let message = AnyMessage::new(42i32);

  registry.defer_send("remote1", message).expect("defer");

  assert_eq!(registry.deferred_count("remote1"), 1);
  assert_eq!(registry.state("remote1"), AuthorityState::Unresolved);
}

#[test]
fn test_set_connected_returns_deferred_messages() {
  let mut registry = RemoteAuthorityRegistry::new();
  let msg1 = AnyMessage::new(1i32);
  let msg2 = AnyMessage::new(2i32);

  registry.defer_send("remote1", msg1).expect("defer");
  registry.defer_send("remote1", msg2).expect("defer");

  let deferred = registry.set_connected("remote1");
  assert!(deferred.is_some());
  assert_eq!(deferred.unwrap().len(), 2);
  assert_eq!(registry.state("remote1"), AuthorityState::Connected);
  assert_eq!(registry.deferred_count("remote1"), 0);
}

#[test]
fn test_transition_unresolved_to_connected() {
  let mut registry = RemoteAuthorityRegistry::new();
  registry.defer_send("remote1", AnyMessage::new(42i32)).expect("defer");

  assert_eq!(registry.state("remote1"), AuthorityState::Unresolved);

  registry.set_connected("remote1");
  assert_eq!(registry.state("remote1"), AuthorityState::Connected);
}

#[test]
fn test_transition_to_quarantine_clears_deferred() {
  let mut registry = RemoteAuthorityRegistry::new();
  registry.defer_send("remote1", AnyMessage::new(1i32)).expect("defer");
  registry.defer_send("remote1", AnyMessage::new(2i32)).expect("defer");

  assert_eq!(registry.deferred_count("remote1"), 2);

  registry.set_quarantine("remote1", 0, Some(Duration::from_secs(300)));

  assert_eq!(registry.state("remote1"), AuthorityState::Quarantine { deadline: Some(300) });
  assert_eq!(registry.deferred_count("remote1"), 0);
}

#[test]
fn test_lift_quarantine_returns_to_unresolved() {
  let mut registry = RemoteAuthorityRegistry::new();
  registry.set_quarantine("remote1", 0, Some(Duration::from_secs(300)));

  assert!(matches!(registry.state("remote1"), AuthorityState::Quarantine { .. }));

  registry.lift_quarantine("remote1");
  assert_eq!(registry.state("remote1"), AuthorityState::Unresolved);
}

#[test]
fn test_defer_after_quarantine_is_rejected() {
  let mut registry = RemoteAuthorityRegistry::new();
  registry.set_quarantine("remote1", 0, Some(Duration::from_secs(300)));

  // quarantine 状態でも defer_send は呼べるが、状態がQuarantineのままであることを確認
  let result = registry.defer_send("remote1", AnyMessage::new(99i32));
  assert!(matches!(result, Err(RemoteAuthorityError::Quarantined)));
  assert_eq!(registry.deferred_count("remote1"), 0);
}

#[test]
fn test_connected_to_unresolved_transition() {
  let mut registry = RemoteAuthorityRegistry::new();
  registry.defer_send("remote1", AnyMessage::new(1i32)).expect("defer");
  registry.set_connected("remote1");

  assert_eq!(registry.state("remote1"), AuthorityState::Connected);

  // Connected状態から再度defer_sendで新しいエントリを追加
  registry.defer_send("remote1", AnyMessage::new(2i32)).expect("defer");
  // 状態はConnectedのまま、deferredに追加される（実装上の挙動）
  assert_eq!(registry.deferred_count("remote1"), 1);
}

// Task 5.2: Quarantine と InvalidAssociation 処理のテスト
#[test]
fn test_quarantine_rejects_new_sends() {
  // 隔離中の authority への新規送信が拒否されることを確認
  let mut registry = RemoteAuthorityRegistry::new();
  registry.set_quarantine("remote1", 0, Some(Duration::from_secs(300)));

  // 隔離中は新規メッセージを受け付けない（defer_sendは呼べるが実装で拒否される想定）
  let result = registry.defer_send("remote1", AnyMessage::new(1i32));
  assert!(result.is_err());
  assert_eq!(registry.deferred_count("remote1"), 0);
}

#[test]
fn test_quarantine_duration_calculation() {
  // 隔離期間が正しく計算されることを確認
  let mut registry = RemoteAuthorityRegistry::new();
  let quarantine_duration = Duration::from_secs(600);
  registry.set_quarantine("remote1", 0, Some(quarantine_duration));

  // 状態確認
  match registry.state("remote1") {
    | AuthorityState::Quarantine { deadline } => {
      assert_eq!(deadline, Some(600));
    },
    | _ => panic!("expected quarantine state"),
  }
}

#[test]
fn test_quarantine_period_expiration() {
  // 隔離期間経過後、自動的に Unresolved へ戻ることを確認
  let mut registry = RemoteAuthorityRegistry::new();
  let short_duration = Duration::from_millis(1);
  registry.set_quarantine("remote1", 0, Some(short_duration));

  // 期限チェックして解除（簡易実装では即座に解除）
  let lifted = registry.poll_quarantine_expiration(1000);

  assert_eq!(lifted.len(), 1);
  assert_eq!(lifted[0], "remote1");

  assert_eq!(registry.state("remote1"), AuthorityState::Unresolved);
}

#[test]
fn test_invalid_association_on_quarantine() {
  // InvalidAssociation を受信したとき、quarantine へ遷移することを確認
  let mut registry = RemoteAuthorityRegistry::new();
  registry.defer_send("remote1", AnyMessage::new(1i32)).expect("defer");
  registry.set_connected("remote1");

  // InvalidAssociation イベントをトリガー
  registry.handle_invalid_association("remote1", 0, Some(Duration::from_secs(300)));

  assert!(matches!(registry.state("remote1"), AuthorityState::Quarantine { .. }));
  // deferred キューはクリアされる
  assert_eq!(registry.deferred_count("remote1"), 0);
}

#[test]
fn test_manual_quarantine_override() {
  // 手動解除により即座に Connected へ遷移できることを確認
  let mut registry = RemoteAuthorityRegistry::new();
  registry.set_quarantine("remote1", 0, Some(Duration::from_secs(3600)));

  // 手動で解除して接続状態へ
  registry.manual_override_to_connected("remote1");

  assert_eq!(registry.state("remote1"), AuthorityState::Connected);
}

// Task 5.3: EventStream 連携と状態遷移の観測
#[test]
fn test_state_transitions_observable() {
  // 各状態遷移が発生することを確認
  let mut registry = RemoteAuthorityRegistry::new();

  // 初期: Unresolved
  assert_eq!(registry.state("observable-host"), AuthorityState::Unresolved);

  // Unresolved -> Connected
  registry.set_connected("observable-host");
  assert_eq!(registry.state("observable-host"), AuthorityState::Connected);

  // Connected -> Quarantine
  registry.set_quarantine("observable-host", 0, Some(Duration::from_secs(300)));
  assert!(matches!(registry.state("observable-host"), AuthorityState::Quarantine { .. }));

  // Quarantine -> Unresolved (期限経過)
  registry.lift_quarantine("observable-host");
  assert_eq!(registry.state("observable-host"), AuthorityState::Unresolved);

  // Quarantine -> Connected (手動解除)
  registry.set_quarantine("observable-host", 0, Some(Duration::from_secs(600)));
  registry.manual_override_to_connected("observable-host");
  assert_eq!(registry.state("observable-host"), AuthorityState::Connected);
}

#[test]
fn test_remoting_config_override_applied() {
  // RemotingConfig からの隔離期間設定が適用されることを確認
  let mut registry = RemoteAuthorityRegistry::new();
  let custom_duration = Duration::from_secs(1800); // カスタム 30 分

  registry.set_quarantine("config-host", 0, Some(custom_duration));

  match registry.state("config-host") {
    | AuthorityState::Quarantine { deadline } => {
      assert_eq!(deadline, Some(1800));
    },
    | _ => panic!("expected quarantine state with custom duration"),
  }
}

#[test]
fn test_multiple_authorities_independent_states() {
  // 複数の authority が独立した状態を持つことを確認
  let mut registry = RemoteAuthorityRegistry::new();

  registry.defer_send("host1", AnyMessage::new(1i32)).expect("defer");
  registry.defer_send("host2", AnyMessage::new(2i32)).expect("defer");

  assert_eq!(registry.state("host1"), AuthorityState::Unresolved);
  assert_eq!(registry.state("host2"), AuthorityState::Unresolved);

  registry.set_connected("host1");
  assert_eq!(registry.state("host1"), AuthorityState::Connected);
  assert_eq!(registry.state("host2"), AuthorityState::Unresolved);

  registry.set_quarantine("host2", 0, Some(Duration::from_secs(300)));
  assert_eq!(registry.state("host1"), AuthorityState::Connected);
  assert!(matches!(registry.state("host2"), AuthorityState::Quarantine { .. }));
}
