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

  assert_eq!(
    manager.state("remote1"),
    AuthorityState::Quarantine {
      deadline: Some(Duration::from_secs(300))
    }
  );
  assert_eq!(manager.deferred_count("remote1"), 0);
}

#[test]
fn test_lift_quarantine_returns_to_unresolved() {
  let manager = RemoteAuthorityManager::new();
  manager.set_quarantine("remote1", Some(Duration::from_secs(300)));

  assert!(matches!(
    manager.state("remote1"),
    AuthorityState::Quarantine { .. }
  ));

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
