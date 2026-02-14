use alloc::{boxed::Box, collections::VecDeque};

use fraktor_utils_rs::core::{
  collections::queue::OverflowPolicy,
  runtime_toolbox::NoStdToolbox,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use crate::core::{
  DynValue, KeepRight, RestartSettings, SourceLogic, StreamBufferConfig, StreamCompletion, StreamDone, StreamDslError,
  StreamError,
  lifecycle::{
    DriveOutcome, SharedKillSwitch, Stream, StreamHandleGeneric, StreamHandleId, StreamSharedGeneric, StreamState,
  },
  mat::{Materialized, Materializer},
  stage::{Sink, Source, StageKind},
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

struct SequenceSourceLogic {
  values: VecDeque<u32>,
}

impl SequenceSourceLogic {
  fn new(values: &[u32]) -> Self {
    let mut queue = VecDeque::with_capacity(values.len());
    queue.extend(values.iter().copied());
    Self { values: queue }
  }
}

impl SourceLogic for SequenceSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.values.pop_front().map(|value| Box::new(value) as DynValue))
  }
}

struct CancelAwareSourceLogic {
  next:         u32,
  cancel_count: ArcShared<SpinSyncMutex<u32>>,
}

impl CancelAwareSourceLogic {
  fn new(cancel_count: ArcShared<SpinSyncMutex<u32>>) -> Self {
    Self { next: 0, cancel_count }
  }
}

impl SourceLogic for CancelAwareSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    self.next = self.next.saturating_add(1);
    Ok(Some(Box::new(self.next)))
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    let mut count = self.cancel_count.lock();
    *count = count.saturating_add(1);
    Ok(())
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
fn materialized_unique_kill_switch_ignores_later_abort_after_shutdown() {
  let source = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new());
  let graph = source.to_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.unique_kill_switch();

  kill_switch.shutdown();
  kill_switch.abort(StreamError::Failed);

  for _ in 0..4 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(materialized.handle().state(), StreamState::Completed);
}

#[test]
fn materialized_shared_kill_switch_shutdown_cancels_upstream_once() {
  let cancel_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let source = Source::<u32, _>::from_logic(StageKind::Custom, CancelAwareSourceLogic::new(cancel_count.clone()));
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
  assert_eq!(*cancel_count.lock(), 1);
}

#[test]
fn materialized_unique_kill_switch_abort_cancels_upstream_once() {
  let cancel_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let source = Source::<u32, _>::from_logic(StageKind::Custom, CancelAwareSourceLogic::new(cancel_count.clone()));
  let graph = source.to_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.unique_kill_switch();

  kill_switch.abort(StreamError::Failed);
  let _ = materialized.handle().drive();

  assert_eq!(materialized.handle().state(), StreamState::Failed);
  assert_eq!(*cancel_count.lock(), 1);
}

#[test]
fn shared_kill_switch_created_before_materialization_controls_multiple_streams() {
  let shared_kill_switch = SharedKillSwitch::new();
  let graph_left = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .to_mat(Sink::ignore(), KeepRight)
    .with_shared_kill_switch(&shared_kill_switch);
  let graph_right = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .to_mat(Sink::ignore(), KeepRight)
    .with_shared_kill_switch(&shared_kill_switch);
  let mut materializer = RecordingMaterializer::default();

  let left = graph_left.run(&mut materializer).expect("left materialize");
  let right = graph_right.run(&mut materializer).expect("right materialize");

  for _ in 0..3 {
    let _ = left.handle().drive();
    let _ = right.handle().drive();
  }
  assert_eq!(left.handle().state(), StreamState::Running);
  assert_eq!(right.handle().state(), StreamState::Running);

  shared_kill_switch.shutdown();
  for _ in 0..8 {
    let _ = left.handle().drive();
    let _ = right.handle().drive();
    if left.handle().state().is_terminal() && right.handle().state().is_terminal() {
      break;
    }
  }

  assert_eq!(left.handle().state(), StreamState::Completed);
  assert_eq!(right.handle().state(), StreamState::Completed);
}

#[test]
fn source_broadcast_duplicates_each_element() {
  let values = Source::single(5_u32).broadcast(2).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32, 5_u32]);
}

#[test]
fn source_empty_completes_without_elements() {
  let values = Source::<u32, _>::empty().collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_from_option_emits_present_value() {
  let values = Source::from_option(Some(7_u32)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_from_option_none_completes_without_elements() {
  let values = Source::<u32, _>::from_option(None).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_from_iterator_emits_values_in_order() {
  let values = Source::from_iterator([1_u32, 2, 3, 4]).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3, 4]);
}

#[test]
fn source_from_iterator_empty_iterator_completes_without_elements() {
  let values = Source::from_iterator(core::iter::empty::<u32>()).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_from_array_emits_values_in_order() {
  let values = Source::from_array([1_u32, 2, 3, 4]).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3, 4]);
}

#[test]
fn source_from_array_empty_array_completes_without_elements() {
  let values = Source::<u32, _>::from_array([]).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
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
  let values = Source::single(5_u32)
    .flat_map_merge(2, Source::single)
    .expect("flat_map_merge")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_flat_map_merge_rejects_zero_breadth() {
  let result = Source::single(1_u32).flat_map_merge(0, Source::single);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "breadth", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_buffer_keeps_single_path_behavior() {
  let values =
    Source::single(5_u32).buffer(2, OverflowPolicy::Block).expect("buffer").collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_buffer_rejects_zero_capacity() {
  let result = Source::single(1_u32).buffer(0, OverflowPolicy::Block);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_async_boundary_keeps_single_path_behavior() {
  let values = Source::single(5_u32).async_boundary().collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_filter_keeps_matching_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .filter(|value| value % 2 == 0)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 4_u32]);
}

#[test]
fn source_filter_not_keeps_non_matching_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .filter_not(|value| value % 2 == 0)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32]);
}

#[test]
fn source_flatten_optional_emits_present_value() {
  let values = Source::single(Some(7_u32)).flatten_optional().collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_flatten_optional_skips_none() {
  let values = Source::single(None::<u32>).flatten_optional().collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_map_async_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .map_async(2, |value| async move { value.saturating_add(1) })
    .expect("map_async")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn source_map_async_rejects_zero_parallelism() {
  let source = Source::single(7_u32);
  let result = source.map_async(0, |value| async move { value });
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "parallelism", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_map_concat_expands_each_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .map_concat(|value: u32| [value, value.saturating_add(10)])
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 11_u32, 2_u32, 12_u32, 3_u32, 13_u32]);
}

#[test]
fn source_map_option_emits_only_present_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .map_option(|value| if value % 2 == 0 { Some(value) } else { None })
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 4_u32]);
}

#[test]
fn source_stateful_map_emits_stateful_results() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .stateful_map(|| {
      let mut sum = 0_u32;
      move |value| {
        sum = sum.saturating_add(value);
        sum
      }
    })
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn source_stateful_map_concat_expands_with_stateful_mapper() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .stateful_map_concat(|| {
      let mut sum = 0_u32;
      move |value| {
        sum = sum.saturating_add(value);
        [sum, sum.saturating_add(100)]
      }
    })
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 101_u32, 3_u32, 103_u32, 6_u32, 106_u32]);
}

#[test]
fn source_drop_skips_first_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .drop(2)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn source_take_limits_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .take(2)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_drop_while_skips_matching_prefix() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .drop_while(|value| *value < 3)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn source_take_while_keeps_matching_prefix() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .take_while(|value| *value < 3)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_take_until_includes_first_matching_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .take_until(|value| *value >= 3)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_grouped_emits_fixed_size_chunks() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]))
    .grouped(2)
    .expect("grouped")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32], vec![5_u32]]);
}

#[test]
fn source_grouped_rejects_zero_size() {
  let result = Source::single(1_u32).grouped(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_sliding_emits_overlapping_windows() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .sliding(3)
    .expect("sliding")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32, 3_u32], vec![2_u32, 3_u32, 4_u32]]);
}

#[test]
fn source_sliding_rejects_zero_size() {
  let result = Source::single(1_u32).sliding(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_scan_emits_initial_and_running_accumulation() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .scan(0_u32, |acc, value| acc + value)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![0_u32, 1_u32, 3_u32, 6_u32]);
}

#[test]
fn source_intersperse_injects_markers() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .intersperse(10_u32, 99_u32, 11_u32)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32, 1_u32, 99_u32, 2_u32, 99_u32, 3_u32, 11_u32]);
}

#[test]
fn source_intersperse_on_empty_stream_emits_start_and_end() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .intersperse(10_u32, 99_u32, 11_u32)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32, 11_u32]);
}

#[test]
fn source_zip_with_index_pairs_each_element_with_index() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[7, 8, 9]))
    .zip_with_index()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![(7_u32, 0_u64), (8_u32, 1_u64), (9_u32, 2_u64)]);
}

#[test]
fn source_group_by_keeps_single_path_behavior() {
  let values = Source::single(5_u32)
    .group_by(4, |value: &u32| value % 2)
    .expect("group_by")
    .merge_substreams()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_group_by_rejects_zero_max_substreams() {
  let result = Source::single(1_u32).group_by(0, |value: &u32| *value);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "max_substreams", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_split_when_emits_single_segment_for_single_element() {
  let values = Source::single(5_u32).split_when(|_| false).into_source().collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_split_after_emits_single_segment_for_single_element() {
  let values = Source::single(5_u32).split_after(|_| false).into_source().collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_split_when_starts_new_segment_with_matching_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .split_when(|value| value % 2 == 0)
    .into_source()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32], vec![2_u32, 3_u32], vec![4_u32]]);
}

#[test]
fn source_split_after_keeps_matching_element_in_current_segment() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .split_after(|value| value % 2 == 0)
    .into_source()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32]]);
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
fn source_merge_substreams_with_parallelism_flattens_single_segment() {
  let values = Source::single(5_u32)
    .split_after(|_| true)
    .merge_substreams_with_parallelism(2)
    .expect("merge_substreams_with_parallelism")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_merge_substreams_with_parallelism_rejects_zero_parallelism() {
  let result = Source::single(5_u32).split_after(|_| true).merge_substreams_with_parallelism(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "parallelism", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_group_by_fails_when_unique_key_count_exceeds_limit() {
  let result = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .group_by(2, |value: &u32| *value)
    .expect("group_by")
    .merge_substreams()
    .collect_values();
  assert_eq!(result, Err(StreamError::SubstreamLimitExceeded { max_substreams: 2 }));
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
fn source_restart_with_settings_keeps_single_path_behavior() {
  let settings = RestartSettings::new(1, 4, 3)
    .with_random_factor_permille(250)
    .with_max_restarts_within_ticks(16)
    .with_jitter_seed(11);
  let values = Source::single(5_u32).restart_source_with_settings(settings).collect_values().expect("collect_values");
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
