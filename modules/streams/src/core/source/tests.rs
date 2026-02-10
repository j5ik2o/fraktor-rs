use alloc::boxed::Box;

use fraktor_utils_rs::core::{collections::queue::OverflowPolicy, runtime_toolbox::NoStdToolbox};

use super::super::{stream::Stream, stream_shared::StreamSharedGeneric};
use crate::core::{
  DriveOutcome, DynValue, KeepRight, Materialized, Materializer, Sink, Source, SourceLogic, StageKind,
  StreamBufferConfig, StreamCompletion, StreamDone, StreamError, StreamHandleGeneric, StreamHandleId, StreamState,
};

struct RecordingMaterializer {
  calls: usize,
}

impl RecordingMaterializer {
  const fn new() -> Self {
    Self { calls: 0 }
  }
}

impl Default for RecordingMaterializer {
  fn default() -> Self {
    Self::new()
  }
}

impl Materializer for RecordingMaterializer {
  type Toolbox = NoStdToolbox;

  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(
    &mut self,
    graph: super::super::RunnableGraph<Mat>,
  ) -> Result<Materialized<Mat, Self::Toolbox>, StreamError> {
    self.calls += 1;
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let shared = StreamSharedGeneric::new(stream);
    let handle = StreamHandleGeneric::new(StreamHandleId::next(), shared);
    Ok(Materialized::new(handle, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

struct EndlessSourceLogic {
  next: u32,
}

impl EndlessSourceLogic {
  const fn new() -> Self {
    Self { next: 0 }
  }
}

impl SourceLogic for EndlessSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    self.next = self.next.saturating_add(1);
    Ok(Some(Box::new(self.next)))
  }
}

#[test]
fn run_with_delegates_to_materializer_and_uses_sink_materialized_value() {
  let (graph, _completion) = Sink::<u32, StreamCompletion<StreamDone>>::ignore().into_parts();
  let marker = 7_u32;
  let sink = Sink::from_graph(graph, marker);
  let source = Source::single(1_u32);
  let mut materializer = RecordingMaterializer::default();
  let materialized = source.run_with(sink, &mut materializer).expect("run_with");
  assert_eq!(materializer.calls, 1);
  assert_eq!(*materialized.materialized(), marker);
}

#[test]
fn materialized_unique_kill_switch_abort_fails_stream() {
  let source = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new());
  let graph = source.to_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.unique_kill_switch();

  kill_switch.abort(StreamError::Failed);
  let _ = materialized.handle().drive();

  assert_eq!(materialized.handle().state(), StreamState::Failed);
}

#[test]
fn materialized_unique_kill_switch_abort_stops_reporting_progress_after_failure() {
  let source = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new());
  let graph = source.to_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.unique_kill_switch();

  kill_switch.abort(StreamError::Failed);
  assert_eq!(materialized.handle().drive(), DriveOutcome::Progressed);
  assert_eq!(materialized.handle().state(), StreamState::Failed);
  assert_eq!(materialized.handle().drive(), DriveOutcome::Idle);
}

#[test]
fn materialized_shared_kill_switch_shutdown_completes_stream() {
  let source = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new());
  let graph = source.to_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.shared_kill_switch();

  kill_switch.shutdown();
  for _ in 0..4 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      break;
    }
  }

  assert_eq!(materialized.handle().state(), StreamState::Completed);
}

#[test]
fn source_broadcast_duplicates_each_element() {
  let values = Source::single(5_u32).broadcast(2).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32, 5_u32]);
}

#[test]
#[should_panic(expected = "fan_out must be greater than zero")]
fn source_broadcast_rejects_zero_fan_out() {
  let _ = Source::single(1_u32).broadcast(0);
}

#[test]
fn source_balance_keeps_single_path_behavior() {
  let values = Source::single(5_u32).balance(1).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "fan_out must be greater than zero")]
fn source_balance_rejects_zero_fan_out() {
  let _ = Source::single(1_u32).balance(0);
}

#[test]
fn source_merge_keeps_single_path_behavior() {
  let values = Source::single(5_u32).merge(1).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn source_merge_rejects_zero_fan_in() {
  let _ = Source::single(1_u32).merge(0);
}

#[test]
fn source_zip_wraps_value_when_single_path() {
  let values = Source::single(5_u32).zip(1).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn source_zip_rejects_zero_fan_in() {
  let _ = Source::single(1_u32).zip(0);
}

#[test]
fn source_concat_keeps_single_path_behavior() {
  let values = Source::single(5_u32).concat(1).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn source_concat_rejects_zero_fan_in() {
  let _ = Source::single(1_u32).concat(0);
}

#[test]
fn source_flat_map_merge_keeps_single_path_behavior() {
  let values = Source::single(5_u32).flat_map_merge(2, Source::single).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "breadth must be greater than zero")]
fn source_flat_map_merge_rejects_zero_breadth() {
  let _ = Source::single(1_u32).flat_map_merge(0, Source::single);
}

#[test]
fn source_buffer_keeps_single_path_behavior() {
  let values = Source::single(5_u32).buffer(2, OverflowPolicy::Block).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "capacity must be greater than zero")]
fn source_buffer_rejects_zero_capacity() {
  let _ = Source::single(1_u32).buffer(0, OverflowPolicy::Block);
}

#[test]
fn source_async_boundary_keeps_single_path_behavior() {
  let values = Source::single(5_u32).async_boundary().collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_group_by_keeps_single_path_behavior() {
  let values = Source::single(5_u32).group_by(4, |value: &u32| value % 2).collect_values().expect("collect_values");
  assert_eq!(values, vec![(1_u32, 5_u32)]);
}

#[test]
#[should_panic(expected = "max_substreams must be greater than zero")]
fn source_group_by_rejects_zero_max_substreams() {
  let _ = Source::single(1_u32).group_by(0, |value: &u32| *value);
}

#[test]
fn source_split_when_emits_single_segment_for_single_element() {
  let values = Source::single(5_u32).split_when(|_| false).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_split_after_emits_single_segment_for_single_element() {
  let values = Source::single(5_u32).split_after(|_| false).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_merge_substreams_flattens_single_segment() {
  let values = Source::single(5_u32).split_after(|_| true).merge_substreams().collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_concat_substreams_flattens_single_segment() {
  let values =
    Source::single(5_u32).split_after(|_| true).concat_substreams().collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_recover_replaces_error_payload_with_fallback() {
  let values = Source::single(Err::<u32, StreamError>(StreamError::Failed))
    .recover(5_u32)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_recover_with_retries_fails_when_retry_budget_is_exhausted() {
  let result =
    Source::single(Err::<u32, StreamError>(StreamError::Failed)).recover_with_retries(0, 5_u32).collect_values();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn source_restart_with_backoff_keeps_single_path_behavior() {
  let values = Source::single(5_u32).restart_source_with_backoff(1, 3).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_supervision_variants_keep_single_path_behavior() {
  let values = Source::single(5_u32)
    .supervision_stop()
    .supervision_resume()
    .supervision_restart()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}
