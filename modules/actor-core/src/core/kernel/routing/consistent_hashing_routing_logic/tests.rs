use alloc::vec::Vec;

use super::super::{
  consistent_hashable_envelope::ConsistentHashableEnvelope,
  consistent_hashing_routing_logic::ConsistentHashingRoutingLogic, routee::Routee, routing_logic::RoutingLogic,
};
use crate::core::kernel::actor::{
  Pid,
  actor_ref::{ActorRef, NullSender},
  messaging::AnyMessage,
};

fn make_routee(id: u64) -> Routee {
  Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(id, 0), NullSender))
}

fn selected_pid(selected: &Routee) -> Pid {
  match selected {
    | Routee::ActorRef(actor_ref) => actor_ref.pid(),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}

fn hash_key_from_u32(message: &AnyMessage) -> u64 {
  u64::from(*message.downcast_ref::<u32>().expect("u32 message"))
}

#[test]
fn new_creates_logic() {
  // Given/When
  let _logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);

  // Then
  // construction succeeds without panic
}

#[test]
fn select_empty_routees_returns_no_routee() {
  // Given
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees: [Routee; 0] = [];
  let message = AnyMessage::new(7_u32);

  // When
  let selected = logic.select(&message, &routees);

  // Then
  assert!(matches!(selected, Routee::NoRoutee));
}

#[test]
fn select_same_hash_key_returns_same_routee() {
  // Given
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees = [make_routee(11), make_routee(22), make_routee(33)];
  let first = AnyMessage::new(5_u32);
  let second = AnyMessage::new(5_u32);

  // When
  let selected_first = logic.select(&first, &routees);
  let selected_second = logic.select(&second, &routees);

  // Then
  assert_eq!(selected_pid(selected_first), selected_pid(selected_second));
}

#[test]
fn select_is_stable_across_routee_order_changes() {
  // Given
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees = Vec::from([make_routee(11), make_routee(22), make_routee(33)]);
  let mut reordered = routees.clone();
  reordered.reverse();
  let message = AnyMessage::new(9_u32);

  // When
  let selected_original = logic.select(&message, &routees);
  let selected_reordered = logic.select(&message, &reordered);

  // Then
  assert_eq!(selected_pid(selected_original), selected_pid(selected_reordered));
}

#[test]
fn envelope_hash_key_takes_precedence_over_mapper() {
  // Given: mapper が呼ばれたら panic するロジック
  let logic = ConsistentHashingRoutingLogic::new(|_: &AnyMessage| -> u64 {
    panic!("mapper must not be called when envelope is provided");
  });
  let routees = [make_routee(11), make_routee(22), make_routee(33)];

  // When: Envelope 付きメッセージを select
  let envelope = ConsistentHashableEnvelope::new(AnyMessage::new(0_u8), 0xABCD_u64);
  let message = AnyMessage::new(envelope);
  let selected = logic.select(&message, &routees);

  // Then: envelope 経路が採用され panic しない（＝ mapper は呼ばれていない）
  let _ = selected_pid(selected);
}

#[test]
fn envelope_with_same_hash_key_selects_same_routee() {
  // Given: mapper 呼び出しは許容（fallback として動作しても別 key で衝突しないよう 0 を返す）
  let logic = ConsistentHashingRoutingLogic::new(|_: &AnyMessage| -> u64 { 0 });
  let routees = [make_routee(11), make_routee(22), make_routee(33)];

  // When: 異なる内部 payload を持つが、hash_key が同じ Envelope を 2 つ作成
  let first = AnyMessage::new(ConsistentHashableEnvelope::new(AnyMessage::new(1_u32), 777_u64));
  let second = AnyMessage::new(ConsistentHashableEnvelope::new(AnyMessage::new(2_u32), 777_u64));

  let selected_first = logic.select(&first, &routees);
  let selected_second = logic.select(&second, &routees);

  // Then: 同じ hash_key から同じ routee が選択される
  assert_eq!(selected_pid(selected_first), selected_pid(selected_second));
}

#[test]
fn no_envelope_falls_back_to_mapper() {
  // Given: Envelope を使わない普通の u32 メッセージ（従来経路）
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees = [make_routee(11), make_routee(22), make_routee(33)];
  let message_a = AnyMessage::new(42_u32);
  let message_b = AnyMessage::new(42_u32);

  // When: 同じ内容のメッセージで 2 度 select
  let selected_a = logic.select(&message_a, &routees);
  let selected_b = logic.select(&message_b, &routees);

  // Then: mapper が呼ばれ同じ routee が選ばれる（＝ mapper 経路が機能している）
  assert_eq!(selected_pid(selected_a), selected_pid(selected_b));
}

#[test]
fn select_minimal_disruption_on_routee_addition() {
  // Given: 3 routees の構成と、そこへ 1 つ追加した 4 routees の構成
  // Pekko ConsistentHashingRoutingLogic の契約「routee 追加時のキー移動は最小限」を検証する。
  // rendezvous hashing の理論では n → n+1 で移動するキーの期待割合は 1/(n+1) = 1/4。
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees_before = [make_routee(11), make_routee(22), make_routee(33)];
  let routees_after = [make_routee(11), make_routee(22), make_routee(33), make_routee(44)];
  let key_count: u32 = 10_000;

  // When: 各キーについて追加前と追加後の選択結果を比較し、移動したキーを数える
  let mut migrated: u32 = 0;
  for key in 0..key_count {
    let message = AnyMessage::new(key);
    let pid_before = selected_pid(logic.select(&message, &routees_before));
    let pid_after = selected_pid(logic.select(&message, &routees_after));
    if pid_before != pid_after {
      migrated += 1;
    }
  }

  // Then: 移動比率 ≈ 1/(n+1) = 0.25 ± 0.05
  //   ・下限側: 0 に近いと「追加した routee が全く選ばれていない」＝ FNV 分散異常の兆候
  //   ・上限側: 1 に近いと「追加で大量移動」＝ 最小限の分断契約違反
  let ratio = f64::from(migrated) / f64::from(key_count);
  let expected = 0.25_f64;
  let tolerance = 0.05_f64;
  assert!(
    (ratio - expected).abs() < tolerance,
    "migration ratio {} not within {} of {} (migrated {} of {} keys)",
    ratio,
    tolerance,
    expected,
    migrated,
    key_count
  );
}

#[test]
fn select_minimal_disruption_on_routee_removal() {
  // Given: 4 routees の構成と、そこから 1 つ除去した 3 routees の構成
  // Pekko ConsistentHashingRoutingLogic の契約「routee 除去時のキー移動は最小限」を検証する。
  // rendezvous hashing の理論では n → n-1 で移動するキーの期待割合は 1/n = 1/4。
  let logic = ConsistentHashingRoutingLogic::new(hash_key_from_u32);
  let routees_before = [make_routee(11), make_routee(22), make_routee(33), make_routee(44)];
  let routees_after = [make_routee(11), make_routee(22), make_routee(33)];
  let key_count: u32 = 10_000;

  // When: 各キーについて除去前と除去後の選択結果を比較し、移動したキーを数える
  let mut migrated: u32 = 0;
  for key in 0..key_count {
    let message = AnyMessage::new(key);
    let pid_before = selected_pid(logic.select(&message, &routees_before));
    let pid_after = selected_pid(logic.select(&message, &routees_after));
    if pid_before != pid_after {
      migrated += 1;
    }
  }

  // Then: 移動比率 ≈ 1/n = 0.25 ± 0.05
  //   ・理論上、除去された routee を top 選択していたキーのみが移動する
  //   ・残存 routee を選択していたキーは移動しない（rendezvous hashing の性質）
  let ratio = f64::from(migrated) / f64::from(key_count);
  let expected = 0.25_f64;
  let tolerance = 0.05_f64;
  assert!(
    (ratio - expected).abs() < tolerance,
    "migration ratio {} not within {} of {} (migrated {} of {} keys)",
    ratio,
    tolerance,
    expected,
    migrated,
    key_count
  );
}
