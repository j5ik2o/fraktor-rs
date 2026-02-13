use alloc::vec::Vec;

use super::FlowSubFlow;
use crate::core::{
  StreamNotUsed,
  stage::{Flow, Source},
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
