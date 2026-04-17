use super::super::{
  consistent_hashing_routing_logic::ConsistentHashingRoutingLogic, routee::Routee, routing_logic::RoutingLogic,
  smallest_mailbox_routing_logic::SmallestMailboxRoutingLogic,
};
use crate::core::kernel::actor::messaging::AnyMessage;

// ---------------------------------------------------------------------------
// 補助実装: FirstRoutingLogic
// ---------------------------------------------------------------------------

/// Test implementation that always selects the first routee.
struct FirstRoutingLogic;

impl RoutingLogic for FirstRoutingLogic {
  fn select<'a>(&self, _message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    if routees.is_empty() {
      static NO_ROUTEE: Routee = Routee::NoRoutee;
      &NO_ROUTEE
    } else {
      &routees[0]
    }
  }
}

fn assert_routing_logic(_logic: &impl RoutingLogic) {}

// ---------------------------------------------------------------------------
// テスト
// ---------------------------------------------------------------------------

#[test]
fn custom_logic_selects_expected_routee() {
  // 前提: FirstRoutingLogic と routee 配列がある
  let logic = FirstRoutingLogic;
  let routees = [Routee::NoRoutee, Routee::NoRoutee];
  let message = AnyMessage::new(1_u32);

  // 実行: routee を選択する
  let selected = logic.select(&message, &routees);

  // 確認: 先頭要素が選択される
  assert_eq!(*selected, routees[0]);
}

#[test]
fn select_returns_reference_to_slice_element() {
  // 前提: FirstRoutingLogic と routee 配列がある
  let logic = FirstRoutingLogic;
  let routees = [Routee::NoRoutee, Routee::NoRoutee];
  let message = AnyMessage::new(2_u32);

  // 実行: routee を選択する
  let selected = logic.select(&message, &routees);

  // 確認: 返却参照は配列の同じ要素を指す
  assert!(core::ptr::eq(selected, &routees[0]));
}

#[test]
fn consistent_hashing_logic_implements_routing_logic() {
  // 前提
  let logic = ConsistentHashingRoutingLogic::new(|message: &AnyMessage| {
    u64::from(*message.downcast_ref::<u32>().expect("u32 message"))
  });

  // 実行/確認
  assert_routing_logic(&logic);
}

#[test]
fn smallest_mailbox_logic_implements_routing_logic() {
  // 前提
  let logic = SmallestMailboxRoutingLogic::new();

  // 実行/確認
  assert_routing_logic(&logic);
}
