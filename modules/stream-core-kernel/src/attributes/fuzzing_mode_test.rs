use alloc::boxed::Box;
use core::any::Any;

use crate::attributes::{Attribute, Attributes, FuzzingMode};

// --- コンストラクタ / アクセサ ---

#[test]
fn new_stores_given_enabled_flag_true() {
  // Given: enabled = true を渡して構築
  let attr = FuzzingMode::new(true);

  // Then: value() は true を返す
  assert!(attr.value());
}

#[test]
fn new_stores_given_enabled_flag_false() {
  // Given: enabled = false を渡して構築
  let attr = FuzzingMode::new(false);

  // Then: value() は false を返す
  assert!(!attr.value());
}

// --- Attribute trait 実装 ---

#[test]
fn as_any_downcast_succeeds() {
  let boxed: Box<dyn Attribute> = Box::new(FuzzingMode::new(true));
  let downcast = boxed.as_any().downcast_ref::<FuzzingMode>();
  assert!(downcast.is_some());
  assert!(downcast.unwrap().value());
}

#[test]
fn clone_box_produces_independent_copy() {
  let boxed: Box<dyn Attribute> = Box::new(FuzzingMode::new(true));
  let cloned = boxed.clone_box();
  let result = cloned.as_any().downcast_ref::<FuzzingMode>().unwrap();
  assert!(result.value());
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  let lhs = FuzzingMode::new(true);
  let rhs = FuzzingMode::new(true);
  assert!(lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  let lhs = FuzzingMode::new(true);
  let rhs = FuzzingMode::new(false);
  assert!(!lhs.eq_attr(&rhs as &dyn Any));
}

// --- 等価性 / Clone / Copy ---

#[test]
fn same_values_are_equal() {
  assert_eq!(FuzzingMode::new(true), FuzzingMode::new(true));
  assert_eq!(FuzzingMode::new(false), FuzzingMode::new(false));
}

#[test]
fn different_values_are_not_equal() {
  assert_ne!(FuzzingMode::new(true), FuzzingMode::new(false));
}

#[test]
fn clone_preserves_value() {
  let original = FuzzingMode::new(true);
  let cloned = original;
  assert_eq!(original, cloned);
}

#[test]
fn copy_semantics_work() {
  let lhs = FuzzingMode::new(false);
  let rhs = lhs;
  assert_eq!(lhs, rhs);
}

// --- Debug フォーマット ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", FuzzingMode::new(true));
  assert!(!debug.is_empty());
}

// --- Attributes::mandatory_attribute<T: MandatoryAttribute> 経由取得 ---

#[test]
fn mandatory_attribute_retrieval_returns_stored_flag_enabled() {
  // Given: enabled=true を保持する Attributes コレクション
  let attrs = Attributes::fuzzing_mode(true);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<FuzzingMode>();

  // Then: 格納した値と等価なインスタンスが取り出せる
  let got = retrieved.expect("flag must be retrievable as mandatory attribute");
  assert!(got.value());
}

#[test]
fn mandatory_attribute_retrieval_returns_stored_flag_disabled() {
  // Given: enabled=false を保持する Attributes コレクション
  let attrs = Attributes::fuzzing_mode(false);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<FuzzingMode>();

  // Then: 格納した false 値が取り出せる
  let got = retrieved.expect("flag must be retrievable as mandatory attribute");
  assert!(!got.value());
}

#[test]
fn mandatory_attribute_returns_none_when_absent() {
  // Given: 当該 attribute を持たない空の Attributes
  let attrs = Attributes::new();

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<FuzzingMode>();

  // Then: None が返る
  assert!(retrieved.is_none());
}
