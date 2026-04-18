use alloc::boxed::Box;
use core::any::Any;

use crate::core::attributes::{Attribute, Attributes, MaxFixedBufferSize};

// --- コンストラクタ / アクセサ ---

#[test]
fn new_stores_given_size() {
  // Given: 明示的な size を渡して構築
  let attr = MaxFixedBufferSize::new(1000);

  // Then: value() は同じ値を返す
  assert_eq!(attr.value(), 1000);
}

#[test]
fn new_allows_zero_size() {
  // Given: size = 0 （境界値）
  let attr = MaxFixedBufferSize::new(0);

  // Then: value() は 0 を返す
  assert_eq!(attr.value(), 0);
}

#[test]
fn new_allows_usize_max() {
  // Given: size = usize::MAX （境界値）
  let attr = MaxFixedBufferSize::new(usize::MAX);

  // Then: value() は usize::MAX を返す
  assert_eq!(attr.value(), usize::MAX);
}

// --- Attribute trait 実装 ---

#[test]
fn as_any_downcast_succeeds() {
  let boxed: Box<dyn Attribute> = Box::new(MaxFixedBufferSize::new(512));
  let downcast = boxed.as_any().downcast_ref::<MaxFixedBufferSize>();
  assert!(downcast.is_some());
  assert_eq!(downcast.unwrap().value(), 512);
}

#[test]
fn clone_box_produces_independent_copy() {
  let boxed: Box<dyn Attribute> = Box::new(MaxFixedBufferSize::new(2048));
  let cloned = boxed.clone_box();
  let result = cloned.as_any().downcast_ref::<MaxFixedBufferSize>().unwrap();
  assert_eq!(result.value(), 2048);
}

#[test]
fn eq_attr_returns_true_for_equal_values() {
  let lhs = MaxFixedBufferSize::new(1024);
  let rhs = MaxFixedBufferSize::new(1024);
  assert!(lhs.eq_attr(&rhs as &dyn Any));
}

#[test]
fn eq_attr_returns_false_for_different_values() {
  let lhs = MaxFixedBufferSize::new(1024);
  let rhs = MaxFixedBufferSize::new(2048);
  assert!(!lhs.eq_attr(&rhs as &dyn Any));
}

// --- 等価性 / Clone / Copy ---

#[test]
fn same_values_are_equal() {
  assert_eq!(MaxFixedBufferSize::new(64), MaxFixedBufferSize::new(64));
}

#[test]
fn different_values_are_not_equal() {
  assert_ne!(MaxFixedBufferSize::new(64), MaxFixedBufferSize::new(128));
}

#[test]
fn clone_preserves_value() {
  let original = MaxFixedBufferSize::new(999);
  let cloned = original;
  assert_eq!(original, cloned);
}

#[test]
fn copy_semantics_work() {
  let lhs = MaxFixedBufferSize::new(1);
  let rhs = lhs;
  assert_eq!(lhs, rhs);
}

// --- Debug フォーマット ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", MaxFixedBufferSize::new(4096));
  assert!(!debug.is_empty());
}

// --- Attributes::mandatory_attribute<T: MandatoryAttribute> 経由取得 ---

#[test]
fn mandatory_attribute_retrieval_returns_stored_size() {
  // Given: size を保持する Attributes コレクション
  let attrs = Attributes::max_fixed_buffer_size(2048);

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<MaxFixedBufferSize>();

  // Then: 格納した値と等価なインスタンスが取り出せる
  let got = retrieved.expect("size must be retrievable as mandatory attribute");
  assert_eq!(got.value(), 2048);
}

#[test]
fn mandatory_attribute_returns_none_when_absent() {
  // Given: 当該 attribute を持たない空の Attributes
  let attrs = Attributes::new();

  // When: mandatory_attribute で取得
  let retrieved = attrs.mandatory_attribute::<MaxFixedBufferSize>();

  // Then: None が返る
  assert!(retrieved.is_none());
}
