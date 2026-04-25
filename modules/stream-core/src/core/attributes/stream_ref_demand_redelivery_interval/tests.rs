use alloc::boxed::Box;
use core::any::Any;

use crate::core::attributes::{Attribute, Attributes, StreamRefDemandRedeliveryInterval};

#[test]
fn new_stores_timeout_ticks() {
  // Given/When: demand redelivery interval を tick 値で構築する
  let attr = StreamRefDemandRedeliveryInterval::new(1);

  // Then: tick 値が保持される
  assert_eq!(attr.timeout_ticks, 1);
}

#[test]
fn new_allows_zero_ticks() {
  // Given/When: timeout_ticks=0 で構築する
  let attr = StreamRefDemandRedeliveryInterval::new(0);

  // Then: 0 が保持される
  assert_eq!(attr.timeout_ticks, 0);
}

#[test]
fn new_allows_max_ticks() {
  // Given/When: timeout_ticks=u32::MAX で構築する
  let attr = StreamRefDemandRedeliveryInterval::new(u32::MAX);

  // Then: 最大値が保持される
  assert_eq!(attr.timeout_ticks, u32::MAX);
}

#[test]
fn as_any_downcast_succeeds() {
  // Given: Attribute trait object として保持する
  let boxed: Box<dyn Attribute> = Box::new(StreamRefDemandRedeliveryInterval::new(3));

  // When: concrete type に downcast する
  let downcast = boxed.as_any().downcast_ref::<StreamRefDemandRedeliveryInterval>();

  // Then: 元の tick 値が取得できる
  assert!(downcast.is_some());
  assert_eq!(downcast.unwrap().timeout_ticks, 3);
}

#[test]
fn clone_box_preserves_timeout_ticks() {
  // Given: Attribute trait object として保持する
  let boxed: Box<dyn Attribute> = Box::new(StreamRefDemandRedeliveryInterval::new(5));

  // When: clone_box する
  let cloned = boxed.clone_box();

  // Then: clone 後も tick 値が保持される
  let result = cloned.as_any().downcast_ref::<StreamRefDemandRedeliveryInterval>().unwrap();
  assert_eq!(result.timeout_ticks, 5);
}

#[test]
fn eq_attr_distinguishes_timeout_ticks() {
  // Given: 同一値と異なる値の attribute
  let lhs = StreamRefDemandRedeliveryInterval::new(3);
  let same = StreamRefDemandRedeliveryInterval::new(3);
  let different = StreamRefDemandRedeliveryInterval::new(4);

  // Then: eq_attr は tick 値で比較する
  assert!(lhs.eq_attr(&same as &dyn Any));
  assert!(!lhs.eq_attr(&different as &dyn Any));
}

#[test]
fn attributes_factory_stores_typed_demand_redelivery_interval() {
  // Given: Attributes factory 経由で構築する
  let attributes = Attributes::stream_ref_demand_redelivery_interval(3);

  // When: typed attribute として取得する
  let got = attributes.get::<StreamRefDemandRedeliveryInterval>();

  // Then: 設定した値が取得できる
  assert!(got.is_some());
  assert_eq!(got.unwrap().timeout_ticks, 3);
}

#[test]
fn attributes_factory_clone_preserves_demand_redelivery_interval() {
  // Given: StreamRef demand redelivery interval attribute を持つ Attributes
  let original = Attributes::stream_ref_demand_redelivery_interval(3);

  // When: Attributes を clone する
  let cloned = original.clone();

  // Then: typed attribute が保持される
  let got = cloned.get::<StreamRefDemandRedeliveryInterval>();
  assert!(got.is_some());
  assert_eq!(got.unwrap().timeout_ticks, 3);
}
