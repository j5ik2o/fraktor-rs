use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

use crate::core::{Flow, Source, StreamNotUsed};

#[test]
fn broadcast_duplicates_each_element() {
  let values = Source::single(7_u32).via(Flow::new().broadcast(2)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32, 7_u32]);
}

#[test]
#[should_panic(expected = "fan_out must be greater than zero")]
fn broadcast_rejects_zero_fan_out() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.broadcast(0);
}

#[test]
fn balance_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().balance(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "fan_out must be greater than zero")]
fn balance_rejects_zero_fan_out() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.balance(0);
}

#[test]
fn merge_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().merge(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn merge_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.merge(0);
}

#[test]
fn zip_wraps_value_when_single_path() {
  let values = Source::single(7_u32).via(Flow::new().zip(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn zip_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.zip(0);
}

#[test]
fn concat_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().concat(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn concat_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.concat(0);
}

#[test]
fn flat_map_merge_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().flat_map_merge(2, Source::single)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "breadth must be greater than zero")]
fn flat_map_merge_rejects_zero_breadth() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.flat_map_merge(0, Source::single);
}

#[test]
fn buffer_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().buffer(2, OverflowPolicy::Block)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "capacity must be greater than zero")]
fn buffer_rejects_zero_capacity() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.buffer(0, OverflowPolicy::Block);
}
