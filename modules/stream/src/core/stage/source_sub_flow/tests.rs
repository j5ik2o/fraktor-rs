use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::SourceSubFlow;
use crate::core::{
  buffer::StreamBufferConfig,
  lifecycle::{Stream, StreamState},
  stage::{DriveOutcome, Sink, Source},
};

impl<Out, Mat> SourceSubFlow<Out, Mat> {
  pub(crate) fn into_source(self) -> Source<Vec<Out>, Mat> {
    self.source
  }
}

fn run_to_completion<Mat>(graph: crate::core::mat::RunnableGraph<Mat>) {
  let (plan, _materialized) = graph.into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");

  let mut idle_budget = 1024_usize;
  let mut drive_budget = 16_384_usize;
  while stream.state() == StreamState::Running {
    assert!(drive_budget > 0, "stream did not reach terminal state");
    drive_budget = drive_budget.saturating_sub(1);
    match stream.drive() {
      | DriveOutcome::Progressed => idle_budget = 1024,
      | DriveOutcome::Idle => {
        assert!(idle_budget > 0, "stream stalled");
        idle_budget = idle_budget.saturating_sub(1);
      },
    }
  }
  assert_eq!(stream.state(), StreamState::Completed);
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

#[test]
fn source_sub_flow_map_clones_stateful_mapper_per_substream() {
  let values = Source::from_array([1_u32, 2, 3, 4])
    .split_after(|value| value % 2 == 0)
    .map({
      let mut sequence = 0_u32;
      move |value| {
        sequence = sequence.saturating_add(1);
        value.saturating_add(sequence.saturating_mul(10))
      }
    })
    .merge_substreams()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![11_u32, 22_u32, 13_u32, 24_u32]);
}

#[test]
fn source_sub_flow_take_while_clones_stateful_predicate_per_substream() {
  let values = Source::from_array([1_u32, 2, 3, 4])
    .split_after(|value| value % 2 == 0)
    .take_while({
      let mut seen = 0_u32;
      move |_value| {
        seen = seen.saturating_add(1);
        seen <= 1
      }
    })
    .merge_substreams()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32]);
}

// --- SubFlow.to テスト ---

#[test]
fn source_sub_flow_to_produces_runnable_graph() {
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let observed_ref = observed.clone();
  let graph =
    Source::from_array([1_u32, 2, 3, 4]).split_after(|value| value % 2 == 0).to(Sink::foreach(move |value| {
      observed_ref.lock().push(value);
    }));

  run_to_completion(graph);

  assert_eq!(*observed.lock(), vec![1_u32, 2, 3, 4]);
}

#[test]
fn source_sub_flow_to_with_map_applies_transformation_before_sink() {
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let observed_ref = observed.clone();
  let graph = Source::from_array([1_u32, 2, 3, 4]).split_after(|value| value % 2 == 0).map(|value| value * 10).to(
    Sink::foreach(move |value| {
      observed_ref.lock().push(value);
    }),
  );

  run_to_completion(graph);

  assert_eq!(*observed.lock(), vec![10_u32, 20, 30, 40]);
}
