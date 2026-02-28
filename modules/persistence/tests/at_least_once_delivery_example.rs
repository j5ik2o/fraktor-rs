//! At-least-once delivery usage example.
//!
//! This example demonstrates how to use the at-least-once delivery helper:
//! - Delivery tracking with unique delivery IDs
//! - Confirmation of received messages
//! - Snapshot save and restore for recovery
//! - Redelivery tick handling pattern
//! - Max unconfirmed limit enforcement

use core::{any::Any, time::Duration};

use fraktor_actor_rs::core::actor::actor_ref::ActorRefGeneric;
use fraktor_persistence_rs::core::{
  AtLeastOnceDelivery, AtLeastOnceDeliveryConfig, AtLeastOnceDeliveryGeneric, RedeliveryTick, UnconfirmedDelivery,
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared, time::TimerInstant};

/// Helper function to create a test payload.
fn create_payload<T: Any + Send + Sync + 'static>(value: T) -> ArcShared<dyn Any + Send + Sync> {
  ArcShared::new(value)
}

/// Helper function to create a null actor reference.
fn null_actor_ref() -> ActorRefGeneric<NoStdToolbox> {
  ActorRefGeneric::null()
}

/// Helper function to create a timer instant.
fn now() -> TimerInstant {
  TimerInstant::from_ticks(100, Duration::from_millis(10))
}

#[test]
fn test_basic_delivery_tracking() {
  // デフォルト設定で AtLeastOnceDelivery を作成
  let mut delivery: AtLeastOnceDelivery<NoStdToolbox> = AtLeastOnceDelivery::new(AtLeastOnceDeliveryConfig::default());

  // 初期状態の確認
  assert_eq!(delivery.current_delivery_id(), 1);
  assert_eq!(delivery.number_of_unconfirmed(), 0);
  assert!(delivery.can_accept_more());

  // 配信を追跡
  let id1 = delivery.next_delivery_id();
  let unconfirmed1 = UnconfirmedDelivery::new(id1, null_actor_ref(), create_payload("message-1"), None, now(), 0);
  delivery.add_unconfirmed(unconfirmed1);

  let id2 = delivery.next_delivery_id();
  let unconfirmed2 = UnconfirmedDelivery::new(id2, null_actor_ref(), create_payload("message-2"), None, now(), 0);
  delivery.add_unconfirmed(unconfirmed2);

  // 状態の確認
  assert_eq!(delivery.number_of_unconfirmed(), 2);
  assert_eq!(delivery.unconfirmed_deliveries().len(), 2);

  // ID が正しく割り当てられていることを確認
  assert_eq!(id1, 1);
  assert_eq!(id2, 2);
  assert_eq!(delivery.current_delivery_id(), 3);
}

#[test]
fn test_delivery_confirmation() {
  let mut delivery: AtLeastOnceDelivery<NoStdToolbox> = AtLeastOnceDelivery::new(AtLeastOnceDeliveryConfig::default());

  // 複数の配信を追加
  for i in 1..=3 {
    let id = delivery.next_delivery_id();
    let unconfirmed =
      UnconfirmedDelivery::new(id, null_actor_ref(), create_payload(format!("message-{}", i)), None, now(), 0);
    delivery.add_unconfirmed(unconfirmed);
  }

  assert_eq!(delivery.number_of_unconfirmed(), 3);

  // 配信 ID 2 を確認
  let confirmed = delivery.confirm_delivery(2);
  assert!(confirmed);
  assert_eq!(delivery.number_of_unconfirmed(), 2);

  // 存在しない ID を確認しようとする
  let not_found = delivery.confirm_delivery(99);
  assert!(!not_found);
  assert_eq!(delivery.number_of_unconfirmed(), 2);

  // 残りの配信を確認
  assert!(delivery.confirm_delivery(1));
  assert!(delivery.confirm_delivery(3));
  assert_eq!(delivery.number_of_unconfirmed(), 0);
}

#[test]
fn test_max_unconfirmed_enforcement() {
  // max_unconfirmed = 3 の設定を作成
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(5), 3, 10, 5);
  let mut delivery: AtLeastOnceDelivery<NoStdToolbox> = AtLeastOnceDelivery::new(config);

  // 最大数まで追加
  for _ in 0..3 {
    assert!(delivery.can_accept_more());
    let id = delivery.next_delivery_id();
    let unconfirmed = UnconfirmedDelivery::new(id, null_actor_ref(), create_payload("msg"), None, now(), 0);
    delivery.add_unconfirmed(unconfirmed);
  }

  // 最大数に達したため、これ以上追加できない
  assert!(!delivery.can_accept_more());

  // 1つ確認すると再び追加可能
  delivery.confirm_delivery(1);
  assert!(delivery.can_accept_more());
}

#[test]
fn test_snapshot_save_and_restore() {
  // アクター1: 配信を追跡
  let mut delivery1: AtLeastOnceDelivery<NoStdToolbox> = AtLeastOnceDelivery::new(AtLeastOnceDeliveryConfig::default());

  for i in 1..=3 {
    let id = delivery1.next_delivery_id();
    let unconfirmed =
      UnconfirmedDelivery::new(id, null_actor_ref(), create_payload(format!("message-{}", i)), None, now(), 0);
    delivery1.add_unconfirmed(unconfirmed);
  }

  // 1つ確認済み
  delivery1.confirm_delivery(2);

  // スナップショットを保存
  let snapshot = delivery1.get_delivery_snapshot();

  // アクター2: スナップショットからリカバリ
  let mut delivery2: AtLeastOnceDelivery<NoStdToolbox> = AtLeastOnceDelivery::new(AtLeastOnceDeliveryConfig::default());
  delivery2.set_delivery_snapshot(snapshot, now());

  // 状態が復元されていることを確認
  assert_eq!(delivery2.current_delivery_id(), 4); // 次の配信 ID
  assert_eq!(delivery2.number_of_unconfirmed(), 2); // 配信 1 と 3 が未確認
  assert_eq!(delivery2.unconfirmed_deliveries()[0].delivery_id(), 1);
  assert_eq!(delivery2.unconfirmed_deliveries()[1].delivery_id(), 3);
}

#[test]
fn test_redelivery_tick_detection() {
  // RedeliveryTick メッセージの検出パターン
  let tick = RedeliveryTick;
  let tick_any: &dyn Any = &tick;

  // RedeliveryTick であることを検出
  assert!(AtLeastOnceDeliveryGeneric::<NoStdToolbox>::is_redelivery_tick(tick_any));

  // 他のメッセージは検出されない
  let other_message = "not a tick";
  let other_any: &dyn Any = &other_message;
  assert!(!AtLeastOnceDeliveryGeneric::<NoStdToolbox>::is_redelivery_tick(other_any));
}

#[test]
fn test_deliveries_to_redeliver() {
  // redelivery_burst_limit = 2 の設定（now() で overdue になるよう interval を短くする）
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_millis(200), 100, 2, 5);
  let mut delivery: AtLeastOnceDelivery<NoStdToolbox> = AtLeastOnceDelivery::new(config);

  // 5つの配信を追加
  for i in 1..=5 {
    let id = delivery.next_delivery_id();
    let unconfirmed =
      UnconfirmedDelivery::new(id, null_actor_ref(), create_payload(format!("message-{}", i)), None, now(), 0);
    delivery.add_unconfirmed(unconfirmed);
  }

  // burst_limit により最初の2つのみ返される
  let to_redeliver = delivery.deliveries_to_redeliver(TimerInstant::from_ticks(200, Duration::from_millis(10)));
  assert_eq!(to_redeliver.len(), 2);
  assert_eq!(to_redeliver[0].delivery_id(), 1);
  assert_eq!(to_redeliver[1].delivery_id(), 2);
}

#[test]
fn test_config_accessors() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(30), 500, 25, 7);
  let delivery: AtLeastOnceDelivery<NoStdToolbox> = AtLeastOnceDelivery::new(config);

  assert_eq!(delivery.config().redeliver_interval(), Duration::from_secs(30));
  assert_eq!(delivery.config().max_unconfirmed(), 500);
  assert_eq!(delivery.config().redelivery_burst_limit(), 25);
  assert_eq!(delivery.config().warn_after_number_of_unconfirmed_attempts(), 7);
}

#[test]
fn test_delivery_with_sender() {
  let mut delivery: AtLeastOnceDelivery<NoStdToolbox> = AtLeastOnceDelivery::new(AtLeastOnceDeliveryConfig::default());

  let id = delivery.next_delivery_id();
  let sender = null_actor_ref();

  // 送信者を指定して配信を作成
  let unconfirmed = UnconfirmedDelivery::new(id, null_actor_ref(), create_payload("message"), Some(sender), now(), 0);
  delivery.add_unconfirmed(unconfirmed);

  // 送信者が設定されていることを確認
  assert!(delivery.unconfirmed_deliveries()[0].sender().is_some());
}
