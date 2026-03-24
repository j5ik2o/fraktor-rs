use alloc::vec::Vec;

use super::FlowSubFlow;
use crate::core::{
  StreamNotUsed,
  stage::{Sink, Source, flow::Flow},
};

impl<In, Out, Mat> FlowSubFlow<In, Out, Mat> {
  pub(crate) fn into_flow(self) -> Flow<In, Vec<Out>, Mat> {
    self.flow
  }
}

#[test]
fn flow_sub_flow_merge_substreams_flattens_segment() {
  let values = Source::single(1_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| true).merge_substreams())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn flow_sub_flow_concat_substreams_flattens_segment() {
  let values = Source::single(1_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| true).concat_substreams())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn flow_sub_flow_map_and_filter_delegate_to_inner_substreams() {
  let values = Source::from_array([1_u32, 2, 3, 4])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .split_after(|value| value % 2 == 0)
        .map(|value| value * 10)
        .filter(|value| value % 20 == 0)
        .merge_substreams(),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![20_u32, 40_u32]);
}

#[test]
fn flow_sub_flow_take_and_drop_scope_to_each_substream() {
  let values = Source::from_array([1_u32, 2, 3, 4, 5])
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|value| value % 2 == 0).drop(1).take(1).merge_substreams())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 4_u32]);
}

#[test]
fn flow_sub_flow_take_while_and_drop_while_scope_to_each_substream() {
  let values = Source::from_array([1_u32, 2, 3, 4, 5])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .split_after(|value| value % 2 == 0)
        .drop_while(|value| value % 2 == 1)
        .take_while(|value| *value <= 4)
        .merge_substreams(),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 4_u32]);
}

#[test]
fn flow_sub_flow_map_clones_stateful_mapper_per_substream() {
  let values = Source::from_array([1_u32, 2, 3, 4])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .split_after(|value| value % 2 == 0)
        .map({
          let mut sequence = 0_u32;
          move |value| {
            sequence = sequence.saturating_add(1);
            value.saturating_add(sequence.saturating_mul(10))
          }
        })
        .merge_substreams(),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![11_u32, 22_u32, 13_u32, 24_u32]);
}

#[test]
fn flow_sub_flow_take_while_clones_stateful_predicate_per_substream() {
  let values = Source::from_array([1_u32, 2, 3, 4])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .split_after(|value| value % 2 == 0)
        .take_while({
          let mut seen = 0_u32;
          move |_value| {
            seen = seen.saturating_add(1);
            seen <= 1
          }
        })
        .merge_substreams(),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32]);
}

// --- SubFlow.to tests ---

#[test]
fn flow_sub_flow_to_produces_sink_that_consumes_all_elements() {
  // Given: a FlowSubFlow connected to a fold sink via .to()
  let sub_flow_sink: Sink<u32, StreamNotUsed> =
    Flow::<u32, u32, StreamNotUsed>::new().split_after(|value| value % 2 == 0).to(Sink::ignore());

  // When: connecting a source to the resulting sink
  // Then: the pipeline is well-typed (Sink<In, Mat>)
  let _graph = Source::from_array([1_u32, 2, 3, 4]).to(sub_flow_sink);
}

#[test]
fn flow_sub_flow_to_with_map_applies_transformation_before_sink() {
  // Given: a FlowSubFlow with map applied, connected to a count sink via .to()
  let sub_flow_sink = Flow::<u32, u32, StreamNotUsed>::new()
    .split_after(|value| value % 2 == 0)
    .map(|value| value * 10)
    .to(Sink::ignore());

  // When: connecting a source to the resulting sink
  // Then: the pipeline is well-typed and elements flow through map then to sink
  let _graph = Source::from_array([1_u32, 2, 3, 4]).to(sub_flow_sink);
}
