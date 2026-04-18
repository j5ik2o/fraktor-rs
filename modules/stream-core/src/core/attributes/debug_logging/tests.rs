use alloc::boxed::Box;
use core::any::Any;

use crate::core::attributes::{Attribute, Attributes, DebugLogging};

// --- コンストラクタ / アクセサ ---

#[test]
fn new_stores_given_enabled_flag_true() {
  // Given: enabled = true を渡して構築
  let attr = DebugLogging::new(true);

  // Then: value() は true を返す
  assert!(attr.value());
}

#[test]
fn new_stores_given_enabled_flag_false() {
  // Given: enabled = false を渡して構築
  let attr = DebugLogging::new(false);

  // Then: value() は false を返す
  assert!(!attr.value());
}

// --- Attribute trait 実装 ---

#[test]
fn as_any_downcast_succeeds() {
  let boxed: Box<dyn Attribute> = Box::new(DebugLogging::new(true));
  let downcast = boxed.as_any().downcast_ref::<DebugLogging>();
  assert!(downcast.is_some());
  assert!(downcast.unwrap().value());
}

#[test]
fn clone_box_produces_independent_copy() {
  let boxed: Box<dyn Attribute> = Box::new(DebugLogging::new(true));
  let cloned = boxed.clone_box();
  let result = cloned.as_any().downcast_ref::<DebugLogging>().unwrap();
  assert!(result.value());
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  let lhs = DebugLogging::new(true);
  let rhs = DebugLogging::new(true);
  assert!(lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  let lhs = DebugLogging::new(true);
  let rhs = DebugLogging::new(false);
  assert!(!lhs.eq_attr(&rhs as &dyn Any));
}

// --- 等価性 / Clone / Copy ---

#[test]
fn same_values_are_equal() {
  assert_eq!(DebugLogging::new(true), DebugLogging::new(true));
  assert_eq!(DebugLogging::new(false), DebugLogging::new(false));
}

#[test]
fn different_values_are_not_equal() {
  assert_ne!(DebugLogging::new(true), DebugLogging::new(false));
}

#[test]
fn clone_preserves_value() {
  let original = DebugLogging::new(true);
  let cloned = original;
  assert_eq!(original, cloned);
}

#[test]
fn copy_semantics_work() {
  let lhs = DebugLogging::new(false);
  let rhs = lhs;
  assert_eq!(lhs, rhs);
}

// --- Debug フォーマット ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", DebugLogging::new(true));
  assert!(!debug.is_empty());
}

// --- Attributes::mandatory_attribute<T: MandatoryAttribute> 経由取得 ---

#[test]
fn mandatory_attribute_retrieval_returns_stored_flag_enabled() {
  // Given: enabled=true を保持する Attributes コレクション
  let attrs = Attributes::debug_logging(true);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<DebugLogging>();

  // Then: 格納した true 値が取り出せる
  let got = retrieved.expect("flag must be retrievable as mandatory attribute");
  assert!(got.value());
}

#[test]
fn mandatory_attribute_retrieval_returns_stored_flag_disabled() {
  // Given: enabled=false を保持する Attributes コレクション
  let attrs = Attributes::debug_logging(false);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<DebugLogging>();

  // Then: 格納した false 値が取り出せる
  let got = retrieved.expect("flag must be retrievable as mandatory attribute");
  assert!(!got.value());
}

#[test]
fn mandatory_attribute_returns_none_when_absent() {
  // Given: 当該 attribute を持たない空の Attributes
  let attrs = Attributes::new();

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<DebugLogging>();

  // Then: None が返る
  assert!(retrieved.is_none());
}
