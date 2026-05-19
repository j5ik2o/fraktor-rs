use alloc::boxed::Box;
use core::any::Any;

use crate::attributes::{Attribute, Attributes, SyncProcessingLimit};

// --- コンストラクタ / アクセサ ---

#[test]
fn new_stores_given_limit() {
  // Given: 明示的な limit を渡して構築
  let attr = SyncProcessingLimit::new(1000);

  // Then: value() は同じ値を返す
  assert_eq!(attr.value(), 1000);
}

#[test]
fn new_allows_zero_limit() {
  // Given: limit = 0 （境界値）
  let attr = SyncProcessingLimit::new(0);

  // Then: value() は 0 を返す
  assert_eq!(attr.value(), 0);
}

#[test]
fn new_allows_usize_max() {
  // Given: limit = usize::MAX （境界値）
  let attr = SyncProcessingLimit::new(usize::MAX);

  // Then: value() は usize::MAX を返す
  assert_eq!(attr.value(), usize::MAX);
}

// --- Attribute trait 実装 ---

#[test]
fn as_any_downcast_succeeds() {
  let boxed: Box<dyn Attribute> = Box::new(SyncProcessingLimit::new(256));
  let downcast = boxed.as_any().downcast_ref::<SyncProcessingLimit>();
  assert!(downcast.is_some());
  assert_eq!(downcast.unwrap().value(), 256);
}

#[test]
fn clone_box_produces_independent_copy() {
  let boxed: Box<dyn Attribute> = Box::new(SyncProcessingLimit::new(1000));
  let cloned = boxed.clone_box();
  let result = cloned.as_any().downcast_ref::<SyncProcessingLimit>().unwrap();
  assert_eq!(result.value(), 1000);
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  let lhs = SyncProcessingLimit::new(1000);
  let rhs = SyncProcessingLimit::new(1000);
  assert!(lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  let lhs = SyncProcessingLimit::new(1000);
  let rhs = SyncProcessingLimit::new(2000);
  assert!(!lhs.eq_attr(&rhs as &dyn Any));
}

// --- 等価性 / Clone / Copy ---

#[test]
fn same_values_are_equal() {
  assert_eq!(SyncProcessingLimit::new(500), SyncProcessingLimit::new(500));
}

#[test]
fn different_values_are_not_equal() {
  assert_ne!(SyncProcessingLimit::new(500), SyncProcessingLimit::new(1000));
}

#[test]
fn clone_preserves_value() {
  let original = SyncProcessingLimit::new(321);
  let cloned = original;
  assert_eq!(original, cloned);
}

#[test]
fn copy_semantics_work() {
  let lhs = SyncProcessingLimit::new(10);
  let rhs = lhs;
  assert_eq!(lhs, rhs);
}

// --- Debug フォーマット ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", SyncProcessingLimit::new(1000));
  assert!(!debug.is_empty());
}

// --- Attributes::mandatory_attribute<T: MandatoryAttribute> 経由取得 ---

#[test]
fn mandatory_attribute_retrieval_returns_stored_limit() {
  // Given: limit を保持する Attributes コレクション
  let attrs = Attributes::sync_processing_limit(1500);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<SyncProcessingLimit>();

  // Then: 格納した値と等価なインスタンスが取り出せる
  let got = retrieved.expect("limit must be retrievable as mandatory attribute");
  assert_eq!(got.value(), 1500);
}

#[test]
fn mandatory_attribute_returns_none_when_absent() {
  // Given: 当該 attribute を持たない空の Attributes
  let attrs = Attributes::new();

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<SyncProcessingLimit>();

  // Then: None が返る
  assert!(retrieved.is_none());
}
