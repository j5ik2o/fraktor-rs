use alloc::boxed::Box;
use core::any::Any;

use crate::core::{
  StreamDslError,
  attributes::{Attribute, Attributes, StreamRefBufferCapacity},
};

#[test]
fn new_stores_positive_capacity() {
  // Given/When: 正の capacity で構築する
  let attr = StreamRefBufferCapacity::new(32).expect("positive capacity must be accepted");

  // Then: capacity が保持される
  assert_eq!(attr.capacity, 32);
}

#[test]
fn new_rejects_zero_capacity() {
  // Given/When: capacity=0 で構築する
  let error = StreamRefBufferCapacity::new(0).expect_err("zero capacity must be rejected");

  // Then: validate_positive_argument と同じ InvalidArgument 契約で失敗する
  assert_eq!(error, StreamDslError::InvalidArgument {
    name:   "capacity",
    value:  0,
    reason: "must be greater than zero",
  });
}

#[test]
fn new_allows_usize_max_capacity() {
  // Given/When: capacity=usize::MAX で構築する
  let attr = StreamRefBufferCapacity::new(usize::MAX).expect("usize::MAX capacity must be accepted");

  // Then: 最大値が保持される
  assert_eq!(attr.capacity, usize::MAX);
}

#[test]
fn as_any_downcast_succeeds() {
  // Given: Attribute trait object として保持する
  let boxed: Box<dyn Attribute> =
    Box::new(StreamRefBufferCapacity::new(32).expect("positive capacity must be accepted"));

  // When: concrete type に downcast する
  let downcast = boxed.as_any().downcast_ref::<StreamRefBufferCapacity>();

  // Then: 元の capacity が取得できる
  assert!(downcast.is_some());
  assert_eq!(downcast.unwrap().capacity, 32);
}

#[test]
fn clone_box_preserves_capacity() {
  // Given: Attribute trait object として保持する
  let boxed: Box<dyn Attribute> =
    Box::new(StreamRefBufferCapacity::new(64).expect("positive capacity must be accepted"));

  // When: clone_box する
  let cloned = boxed.clone_box();

  // Then: clone 後も capacity が保持される
  let result = cloned.as_any().downcast_ref::<StreamRefBufferCapacity>().unwrap();
  assert_eq!(result.capacity, 64);
}

#[test]
fn eq_attr_distinguishes_capacity() {
  // Given: 同一値と異なる値の attribute
  let lhs = StreamRefBufferCapacity::new(32).expect("positive capacity must be accepted");
  let same = StreamRefBufferCapacity::new(32).expect("positive capacity must be accepted");
  let different = StreamRefBufferCapacity::new(64).expect("positive capacity must be accepted");

  // Then: eq_attr は capacity で比較する
  assert!(lhs.eq_attr(&same as &dyn Any));
  assert!(!lhs.eq_attr(&different as &dyn Any));
}

#[test]
fn attributes_factory_stores_typed_buffer_capacity() {
  // Given: Attributes factory 経由で構築する
  let attributes = Attributes::stream_ref_buffer_capacity(32).expect("positive capacity must be accepted");

  // When: typed attribute として取得する
  let got = attributes.get::<StreamRefBufferCapacity>();

  // Then: 設定した値が取得できる
  assert!(got.is_some());
  assert_eq!(got.unwrap().capacity, 32);
}

#[test]
fn attributes_factory_rejects_zero_capacity() {
  // Given/When: Attributes factory に capacity=0 を渡す
  let error = Attributes::stream_ref_buffer_capacity(0).expect_err("zero capacity must be rejected");

  // Then: 不正な BufferCapacity attribute は保持されない
  assert_eq!(error, StreamDslError::InvalidArgument {
    name:   "capacity",
    value:  0,
    reason: "must be greater than zero",
  });
}
