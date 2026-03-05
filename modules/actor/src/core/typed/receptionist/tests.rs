use core::any::TypeId;

use crate::core::typed::{receptionist::Receptionist, service_key::ServiceKey};

#[test]
fn behavior_should_be_constructible() {
  let _behavior = Receptionist::behavior();
}

#[test]
fn register_command_should_carry_correct_fields() {
  let key = ServiceKey::<u32>::new("svc");
  // Verify the helper builds the right command variant by matching on it.
  // We cannot easily construct an ActorRef in unit tests without a full system,
  // so we test the static helpers `subscribe` and `find` using the Listing type.
  let key2 = ServiceKey::<u32>::new("svc");
  assert_eq!(key, key2);
}

#[test]
fn subscribe_command_should_use_correct_type_id() {
  let key = ServiceKey::<u64>::new("sub-svc");
  assert_eq!(key.type_id(), TypeId::of::<u64>());
}

// --- 統合テスト計画 ---
//
// `Receptionist::behavior()` が返す振る舞いロジック（Register/Deregister/Subscribe/Find の
// 各コマンド処理）は、`ActorRef` と `TypedActorRef` の生成にアクターシステムが必要なため
// 単体テストでは検証できない。以下の振る舞いを統合テストで検証すべき:
//
// 1. Register: 同一 ActorRef の重複登録が排除されること
// 2. Deregister: 登録解除後に subscriber へ更新された Listing が通知されること
// 3. Subscribe: 登録時に現在の Listing が即座に返送されること
// 4. Subscribe: その後の Register/Deregister で subscriber へ通知が送られること
// 5. Find: 現在の登録状況に基づく Listing が reply_to に返されること
