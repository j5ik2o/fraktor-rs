use alloc::boxed::Box;
use core::any::Any;

use crate::attributes::{Attribute, Attributes, StreamRefFinalTerminationSignalDeadline};

#[test]
fn new_stores_timeout_ticks() {
  // Given/When: final termination signal deadline を tick 値で構築する
  let attr = StreamRefFinalTerminationSignalDeadline::new(2);

  // Then: tick 値が保持される
  assert_eq!(attr.timeout_ticks, 2);
}

#[test]
fn new_allows_zero_ticks() {
  // Given/When: timeout_ticks=0 で構築する
  let attr = StreamRefFinalTerminationSignalDeadline::new(0);

  // Then: 0 が保持される
  assert_eq!(attr.timeout_ticks, 0);
}

#[test]
fn new_allows_max_ticks() {
  // Given/When: timeout_ticks=u32::MAX で構築する
  let attr = StreamRefFinalTerminationSignalDeadline::new(u32::MAX);

  // Then: 最大値が保持される
  assert_eq!(attr.timeout_ticks, u32::MAX);
}

#[test]
fn as_any_downcast_succeeds() {
  // Given: Attribute trait object として保持する
  let boxed: Box<dyn Attribute> = Box::new(StreamRefFinalTerminationSignalDeadline::new(2));

  // When: concrete type に downcast する
  let downcast = boxed.as_any().downcast_ref::<StreamRefFinalTerminationSignalDeadline>();

  // Then: 元の tick 値が取得できる
  assert!(downcast.is_some());
  assert_eq!(downcast.unwrap().timeout_ticks, 2);
}

#[test]
fn clone_box_preserves_timeout_ticks() {
  // Given: Attribute trait object として保持する
  let boxed: Box<dyn Attribute> = Box::new(StreamRefFinalTerminationSignalDeadline::new(6));

  // When: clone_box する
  let cloned = boxed.clone_box();

  // Then: clone 後も tick 値が保持される
  let result = cloned.as_any().downcast_ref::<StreamRefFinalTerminationSignalDeadline>().unwrap();
  assert_eq!(result.timeout_ticks, 6);
}

#[test]
fn eq_attr_distinguishes_timeout_ticks() {
  // Given: 同一値と異なる値の attribute
  let lhs = StreamRefFinalTerminationSignalDeadline::new(2);
  let same = StreamRefFinalTerminationSignalDeadline::new(2);
  let different = StreamRefFinalTerminationSignalDeadline::new(3);

  // Then: eq_attr は tick 値で比較する
  assert!(lhs.eq_attr(&same as &dyn Any));
  assert!(!lhs.eq_attr(&different as &dyn Any));
}

#[test]
fn attributes_factory_stores_typed_final_termination_signal_deadline() {
  // Given: Attributes factory 経由で構築する
  let attributes = Attributes::stream_ref_final_termination_signal_deadline(2);

  // When: typed attribute として取得する
  let got = attributes.get::<StreamRefFinalTerminationSignalDeadline>();

  // Then: 設定した値が取得できる
  assert!(got.is_some());
  assert_eq!(got.unwrap().timeout_ticks, 2);
}

#[test]
fn attributes_factory_clone_preserves_final_termination_signal_deadline() {
  // Given: StreamRef final termination signal deadline attribute を持つ Attributes
  let original = Attributes::stream_ref_final_termination_signal_deadline(2);

  // When: Attributes を clone する
  let cloned = original.clone();

  // Then: typed attribute が保持される
  let got = cloned.get::<StreamRefFinalTerminationSignalDeadline>();
  assert!(got.is_some());
  assert_eq!(got.unwrap().timeout_ticks, 2);
}
