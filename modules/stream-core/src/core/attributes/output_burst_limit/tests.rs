use alloc::boxed::Box;
use core::any::Any;

use crate::core::attributes::{Attribute, Attributes, OutputBurstLimit};

// --- コンストラクタ / アクセサ ---

#[test]
fn new_stores_given_limit() {
  // Given: 明示的な limit を渡して構築
  let attr = OutputBurstLimit::new(16);

  // Then: value() は同じ値を返す
  assert_eq!(attr.value(), 16);
}

#[test]
fn new_allows_zero_limit() {
  // Given: limit = 0 （境界値）
  let attr = OutputBurstLimit::new(0);

  // Then: value() は 0 を返す
  assert_eq!(attr.value(), 0);
}

#[test]
fn new_allows_usize_max() {
  // Given: limit = usize::MAX （境界値）
  let attr = OutputBurstLimit::new(usize::MAX);

  // Then: value() は usize::MAX を返す
  assert_eq!(attr.value(), usize::MAX);
}

// --- Attribute trait 実装 ---

#[test]
fn as_any_downcast_succeeds() {
  let boxed: Box<dyn Attribute> = Box::new(OutputBurstLimit::new(8));
  let downcast = boxed.as_any().downcast_ref::<OutputBurstLimit>();
  assert!(downcast.is_some());
  assert_eq!(downcast.unwrap().value(), 8);
}

#[test]
fn clone_box_produces_independent_copy() {
  let boxed: Box<dyn Attribute> = Box::new(OutputBurstLimit::new(42));
  let cloned = boxed.clone_box();
  let result = cloned.as_any().downcast_ref::<OutputBurstLimit>().unwrap();
  assert_eq!(result.value(), 42);
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  let lhs = OutputBurstLimit::new(16);
  let rhs = OutputBurstLimit::new(16);
  assert!(lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  let lhs = OutputBurstLimit::new(16);
  let rhs = OutputBurstLimit::new(32);
  assert!(!lhs.eq_attr(&rhs as &dyn Any));
}

// --- 等価性 / Clone / Copy ---

#[test]
fn same_values_are_equal() {
  assert_eq!(OutputBurstLimit::new(7), OutputBurstLimit::new(7));
}

#[test]
fn different_values_are_not_equal() {
  assert_ne!(OutputBurstLimit::new(7), OutputBurstLimit::new(8));
}

#[test]
fn clone_preserves_value() {
  let original = OutputBurstLimit::new(123);
  let cloned = original;
  assert_eq!(original, cloned);
}

#[test]
fn copy_semantics_work() {
  let lhs = OutputBurstLimit::new(5);
  let rhs = lhs;
  assert_eq!(lhs, rhs);
}

// --- Debug フォーマット ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", OutputBurstLimit::new(64));
  assert!(!debug.is_empty());
}

// --- Attributes::mandatory_attribute<T: MandatoryAttribute> 経由取得 ---

#[test]
fn mandatory_attribute_retrieval_returns_stored_limit() {
  // Given: limit を保持する Attributes コレクション
  let attrs = Attributes::output_burst_limit(25);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<OutputBurstLimit>();

  // Then: 格納した値と等価なインスタンスが取り出せる
  let got = retrieved.expect("limit must be retrievable as mandatory attribute");
  assert_eq!(got.value(), 25);
}

#[test]
fn mandatory_attribute_returns_none_when_absent() {
  // Given: 当該 attribute を持たない空の Attributes
  let attrs = Attributes::new();

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<OutputBurstLimit>();

  // Then: None が返る
  assert!(retrieved.is_none());
}
