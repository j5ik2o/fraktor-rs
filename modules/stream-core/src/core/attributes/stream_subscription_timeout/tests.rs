use alloc::boxed::Box;
use core::any::Any;

use crate::core::{
  attributes::{Attribute, Attributes, StreamSubscriptionTimeout},
  stream_subscription_timeout_termination_mode::StreamSubscriptionTimeoutTerminationMode,
};

// --- コンストラクタ / アクセサ ---

#[test]
fn new_creates_with_given_values() {
  // Given: ticks=42, mode=Cancel を指定して構築
  let attr = StreamSubscriptionTimeout::new(42, StreamSubscriptionTimeoutTerminationMode::Cancel);

  // Then: 各フィールドアクセサが格納値を返す
  assert_eq!(attr.timeout_ticks, 42);
  assert_eq!(attr.termination_mode, StreamSubscriptionTimeoutTerminationMode::Cancel);
}

#[test]
fn new_allows_zero_ticks() {
  // Given: timeout_ticks=0 でも構築できる（境界値）
  let attr = StreamSubscriptionTimeout::new(0, StreamSubscriptionTimeoutTerminationMode::Noop);

  // Then: 0 が保持される
  assert_eq!(attr.timeout_ticks, 0);
  assert_eq!(attr.termination_mode, StreamSubscriptionTimeoutTerminationMode::Noop);
}

#[test]
fn new_allows_max_ticks() {
  // Given: timeout_ticks=u32::MAX でも構築できる
  let attr = StreamSubscriptionTimeout::new(u32::MAX, StreamSubscriptionTimeoutTerminationMode::Warn);

  // Then: u32::MAX が保持される
  assert_eq!(attr.timeout_ticks, u32::MAX);
}

// --- Attribute trait 実装 ---

#[test]
fn as_any_downcast_succeeds() {
  let boxed: Box<dyn Attribute> =
    Box::new(StreamSubscriptionTimeout::new(100, StreamSubscriptionTimeoutTerminationMode::Cancel));
  let downcast = boxed.as_any().downcast_ref::<StreamSubscriptionTimeout>();
  assert!(downcast.is_some());
  let result = downcast.unwrap();
  assert_eq!(result.timeout_ticks, 100);
  assert_eq!(result.termination_mode, StreamSubscriptionTimeoutTerminationMode::Cancel);
}

#[test]
fn clone_box_produces_independent_copy() {
  let boxed: Box<dyn Attribute> =
    Box::new(StreamSubscriptionTimeout::new(50, StreamSubscriptionTimeoutTerminationMode::Warn));
  let cloned = boxed.clone_box();
  let result = cloned.as_any().downcast_ref::<StreamSubscriptionTimeout>().unwrap();
  assert_eq!(result.timeout_ticks, 50);
  assert_eq!(result.termination_mode, StreamSubscriptionTimeoutTerminationMode::Warn);
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  let lhs = StreamSubscriptionTimeout::new(10, StreamSubscriptionTimeoutTerminationMode::Cancel);
  let rhs = StreamSubscriptionTimeout::new(10, StreamSubscriptionTimeoutTerminationMode::Cancel);
  assert!(lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn eq_attr_returns_false_for_different_ticks() {
  let lhs = StreamSubscriptionTimeout::new(10, StreamSubscriptionTimeoutTerminationMode::Cancel);
  let rhs = StreamSubscriptionTimeout::new(20, StreamSubscriptionTimeoutTerminationMode::Cancel);
  assert!(!lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn eq_attr_returns_false_for_different_modes() {
  let lhs = StreamSubscriptionTimeout::new(10, StreamSubscriptionTimeoutTerminationMode::Cancel);
  let rhs = StreamSubscriptionTimeout::new(10, StreamSubscriptionTimeoutTerminationMode::Warn);
  assert!(!lhs.eq_attr(&rhs as &dyn Any));
}

// --- 等価性 / Clone ---

#[test]
fn same_values_are_equal() {
  assert_eq!(
    StreamSubscriptionTimeout::new(7, StreamSubscriptionTimeoutTerminationMode::Noop),
    StreamSubscriptionTimeout::new(7, StreamSubscriptionTimeoutTerminationMode::Noop)
  );
}

#[test]
fn different_values_are_not_equal() {
  assert_ne!(
    StreamSubscriptionTimeout::new(7, StreamSubscriptionTimeoutTerminationMode::Noop),
    StreamSubscriptionTimeout::new(7, StreamSubscriptionTimeoutTerminationMode::Cancel)
  );
}

#[test]
fn clone_preserves_values() {
  let original = StreamSubscriptionTimeout::new(33, StreamSubscriptionTimeoutTerminationMode::Warn);
  let cloned = original.clone();
  assert_eq!(cloned, original);
}

// --- Debug フォーマット ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", StreamSubscriptionTimeout::new(1, StreamSubscriptionTimeoutTerminationMode::Noop));
  assert!(!debug.is_empty());
}

// --- Attributes::mandatory_attribute<T: MandatoryAttribute> 経由取得 ---

#[test]
fn mandatory_attribute_retrieval_returns_stored_value() {
  // Given: ticks=128, Cancel mode を保持する Attributes コレクション
  let attrs = Attributes::stream_subscription_timeout(128, StreamSubscriptionTimeoutTerminationMode::Cancel);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<StreamSubscriptionTimeout>();

  // Then: 格納値と等価なインスタンスが取り出せる
  let got = retrieved.expect("StreamSubscriptionTimeout must be retrievable as mandatory attribute");
  assert_eq!(got.timeout_ticks, 128);
  assert_eq!(got.termination_mode, StreamSubscriptionTimeoutTerminationMode::Cancel);
}

#[test]
fn mandatory_attribute_retrieval_preserves_each_termination_mode_variant() {
  // Given: 3 種類の termination_mode を順に保持して取得
  for mode in [
    StreamSubscriptionTimeoutTerminationMode::Noop,
    StreamSubscriptionTimeoutTerminationMode::Warn,
    StreamSubscriptionTimeoutTerminationMode::Cancel,
  ] {
    let attrs = Attributes::stream_subscription_timeout(10, mode);
    let got = attrs
      .mandatory_attribute::<StreamSubscriptionTimeout>()
      .expect("StreamSubscriptionTimeout must be retrievable as mandatory attribute");
    assert_eq!(got.termination_mode, mode);
  }
}

#[test]
fn mandatory_attribute_returns_none_when_absent() {
  // Given: 当該 attribute を持たない空の Attributes
  let attrs = Attributes::new();

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<StreamSubscriptionTimeout>();

  // Then: None が返る
  assert!(retrieved.is_none());
}
