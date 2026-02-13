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
