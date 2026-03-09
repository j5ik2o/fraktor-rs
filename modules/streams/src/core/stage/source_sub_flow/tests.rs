use alloc::vec::Vec;

use super::SourceSubFlow;
use crate::core::stage::Source;

impl<Out, Mat> SourceSubFlow<Out, Mat> {
  pub(crate) fn into_source(self) -> Source<Vec<Out>, Mat> {
    self.source
  }
}

#[test]
fn source_sub_flow_merge_substreams_flattens_segment() {
  let values = Source::single(1_u32).split_after(|_| true).merge_substreams().collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn source_sub_flow_concat_substreams_flattens_segment() {
  let values =
    Source::single(1_u32).split_after(|_| true).concat_substreams().collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn source_sub_flow_map_and_filter_delegate_to_inner_substreams() {
  let values = Source::from_array([1_u32, 2, 3, 4])
    .split_after(|value| value % 2 == 0)
    .map(|value| value * 10)
    .filter(|value| value % 20 == 0)
    .merge_substreams()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![20_u32, 40_u32]);
}

#[test]
fn source_sub_flow_take_and_drop_scope_to_each_substream() {
  let values = Source::from_array([1_u32, 2, 3, 4, 5])
    .split_after(|value| value % 2 == 0)
    .drop(1)
    .take(1)
    .merge_substreams()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 4_u32]);
}

#[test]
fn source_sub_flow_take_while_and_drop_while_scope_to_each_substream() {
  let values = Source::from_array([1_u32, 2, 3, 4, 5])
    .split_after(|value| value % 2 == 0)
    .drop_while(|value| value % 2 == 1)
    .take_while(|value| *value <= 4)
    .merge_substreams()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 4_u32]);
}
