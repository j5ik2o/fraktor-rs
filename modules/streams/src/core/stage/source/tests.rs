use alloc::{boxed::Box, collections::VecDeque};
use core::{future::ready, marker::PhantomData};

use fraktor_utils_rs::core::{
  collections::queue::OverflowPolicy,
  runtime_toolbox::NoStdToolbox,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use crate::core::{
  Completion, DynValue, KeepBoth, KeepLeft, KeepRight, RestartSettings, SourceLogic, StreamBufferConfig,
  StreamCompletion, StreamDone, StreamDslError, StreamError, StreamNotUsed,
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
fn source_map_materialized_value_transforms_materialized_value_and_keeps_data_path_behavior() {
  let (_graph, materialized) = Source::single(1_u32).map_materialized_value(|_| 99_u32).into_parts();
  assert_eq!(materialized, 99_u32);

  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .map_materialized_value(|_| 42_u32)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);

  let graph = Source::single(7_u32).map_materialized_value(|_| 55_u32).to_mat(Sink::ignore(), KeepLeft);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(*materialized.materialized(), 55_u32);
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
  let values = Source::single(5_u32).broadcast(2).expect("broadcast").collect_values().expect("collect_values");
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
fn source_from_alias_emits_values_in_order() {
  let values = Source::from([4_u32, 5, 6]).collect_values().expect("collect_values");
  assert_eq!(values, vec![4_u32, 5, 6]);
}

#[test]
fn source_failed_returns_error_on_collection() {
  let values = Source::<u32, _>::failed(StreamError::Failed).collect_values();
  assert_eq!(values, Err(StreamError::Failed));
}

#[test]
fn source_never_with_take_returns_would_block() {
  let values = Source::<u32, _>::never().take(1).collect_values();
  assert_eq!(values, Err(StreamError::WouldBlock));
}

#[test]
fn source_range_emits_inclusive_sequence() {
  let values = Source::range(2, 5).collect_values().expect("collect_values");
  assert_eq!(values, vec![2, 3, 4, 5]);
}

#[test]
fn source_range_descending_emits_reverse_sequence() {
  let values = Source::range(5, 2).collect_values().expect("collect_values");
  assert_eq!(values, vec![5, 4, 3, 2]);
}

#[test]
fn source_repeat_with_take_limits_elements() {
  let mut logic = super::RepeatSourceLogic { value: 9_u32 };
  let mut values = Vec::new();
  for _ in 0..4 {
    let value = logic.pull().expect("pull").expect("value");
    values.push(*value.downcast::<u32>().expect("u32 value"));
  }
  assert_eq!(values, vec![9_u32, 9, 9, 9]);
}

#[test]
fn source_cycle_repeats_input_sequence() {
  let mut logic = super::CycleSourceLogic { values: vec![1_u32, 2, 3], index: 0 };
  let mut values = Vec::new();
  for _ in 0..7 {
    let value = logic.pull().expect("pull").expect("value");
    values.push(*value.downcast::<u32>().expect("u32 value"));
  }
  assert_eq!(values, vec![1_u32, 2, 3, 1, 2, 3, 1]);
}

#[test]
fn source_cycle_empty_values_completes_without_elements() {
  let values = Source::<u32, _>::cycle(core::iter::empty::<u32>()).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_iterate_emits_progressive_values() {
  let mut logic = super::IterateSourceLogic { current: 1_u32, func: |value| value + 2 };
  let mut values = Vec::new();
  for _ in 0..4 {
    let value = logic.pull().expect("pull").expect("value");
    values.push(*value.downcast::<u32>().expect("u32 value"));
  }
  assert_eq!(values, vec![1_u32, 3, 5, 7]);
}

#[test]
fn source_as_source_with_context_attaches_unit_context() {
  let values =
    Source::from_array([1_u32, 2_u32]).as_source_with_context().as_source().collect_values().expect("collect_values");
  assert_eq!(values, vec![((), 1_u32), ((), 2_u32)]);
}

#[test]
fn source_actor_ref_alias_emits_values() {
  let values = Source::actor_ref([1_u32, 2_u32]).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_actor_ref_with_backpressure_alias_emits_values() {
  let values = Source::actor_ref_with_backpressure([3_u32, 4_u32]).collect_values().expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn source_sink_alias_exposes_sink_endpoint() {
  let values = Source::<u32, _>::sink().as_publisher().collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

// --- watch_termination tests ---

#[test]
fn source_watch_termination_mat_keep_left_passes_elements_through() {
  let values =
    Source::from_array([5_u32, 6_u32]).watch_termination_mat(KeepLeft).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_watch_termination_mat_keep_right_exposes_completion_handle() {
  let source = Source::from_array([1_u32, 2_u32]).watch_termination_mat(KeepRight);
  let completion = source.map_materialized_value(|c| {
    assert_eq!(c.poll(), Completion::Pending);
    c
  });
  let values = completion.collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_watch_termination_mat_keep_both() {
  let (_graph, (left, right)) = Source::<u32, StreamNotUsed>::empty().watch_termination_mat(KeepBoth).into_parts();
  assert_eq!(left, StreamNotUsed::new());
  assert_eq!(right.poll(), Completion::Pending);
}

#[test]
fn source_combine_selects_first_available_source() {
  let values = Source::combine([Source::from_array([1_u32, 2_u32]), Source::from_array([9_u32])])
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_from_java_stream_alias_emits_values() {
  let values = Source::from_java_stream([3_u32, 4_u32]).collect_values().expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn source_from_publisher_alias_emits_values() {
  let values = Source::from_publisher([5_u32, 6_u32]).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_future_alias_emits_when_ready() {
  let values = Source::future(ready(7_u32)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_completion_stage_alias_emits_when_ready() {
  let values = Source::completion_stage(ready(8_u32)).collect_values().expect("collect_values");
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn source_lazy_future_alias_emits_when_ready() {
  let values = Source::lazy_future(|| ready(9_u32)).collect_values().expect("collect_values");
  assert_eq!(values, vec![9_u32]);
}

#[test]
fn source_lazy_single_alias_emits_factory_value() {
  let values = Source::lazy_single(|| 10_u32).collect_values().expect("collect_values");
  assert_eq!(values, vec![10_u32]);
}

#[test]
fn source_lazy_source_emits_all_elements_from_factory() {
  let values = Source::lazy_source(|| Source::from_array([1_u32, 2, 3])).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn source_lazy_source_defers_factory_call() {
  let called = ArcShared::new(SpinSyncMutex::new(false));
  let called_clone = called.clone();
  let source = Source::lazy_source(move || {
    *called_clone.lock() = true;
    Source::from_array([42_u32])
  });
  // ファクトリはまだ呼ばれていない
  assert!(!*called.lock());
  let values = source.collect_values().expect("collect_values");
  // ファクトリが呼ばれ、値が取得される
  assert!(*called.lock());
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn source_lazy_source_with_empty_factory_completes_immediately() {
  let values = Source::<u32, _>::lazy_source(Source::empty).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_lazy_source_with_mapped_source_emits_transformed() {
  let values =
    Source::lazy_source(|| Source::from_array([1_u32, 2, 3]).map(|v| v * 10)).collect_values().expect("collect_values");
  assert_eq!(values, vec![10_u32, 20, 30]);
}

#[test]
fn source_maybe_alias_matches_from_option_behavior() {
  let values = Source::maybe(Some(11_u32)).collect_values().expect("collect_values");
  assert_eq!(values, vec![11_u32]);
}

#[test]
fn source_queue_alias_matches_iterator_behavior() {
  let values = Source::queue([12_u32, 13_u32]).collect_values().expect("collect_values");
  assert_eq!(values, vec![12_u32, 13_u32]);
}

#[test]
fn source_tick_accepts_positive_interval() {
  let source = Source::tick(1, 1, 14_u32);
  assert!(source.is_ok());
}

#[test]
fn source_tick_rejects_zero_interval() {
  let source = Source::tick(1, 0, 14_u32);
  assert!(matches!(
    source,
    Err(StreamDslError::InvalidArgument { name: "interval_ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_unfold_emits_state_progression() {
  let values = Source::unfold(0_u32, |state| {
    if state >= 3 {
      return None;
    }
    Some((state + 1, state))
  })
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![0_u32, 1_u32, 2_u32]);
}

#[test]
fn source_unfold_async_emits_state_progression() {
  let values = Source::unfold_async(0_u32, |state| async move {
    if state >= 3 {
      return None;
    }
    Some((state + 1, state))
  })
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![0_u32, 1_u32, 2_u32]);
}

#[test]
fn source_zip_n_alias_wraps_values_by_fan_in() {
  let values = Source::single(15_u32).zip_n(1).expect("zip_n").collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![15_u32]]);
}

#[test]
fn source_zip_with_n_alias_maps_zipped_values() {
  let values = Source::single(16_u32)
    .zip_with_n(1, |items: Vec<u32>| items.into_iter().sum::<u32>())
    .expect("zip_with_n")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![16_u32]);
}

#[test]
fn source_from_input_stream_alias_emits_values() {
  let values = Source::from_input_stream([17_u32, 18_u32]).collect_values().expect("collect_values");
  assert_eq!(values, vec![17_u32, 18_u32]);
}

#[test]
fn source_from_output_stream_alias_emits_values() {
  let values = Source::from_output_stream([19_u32, 20_u32]).collect_values().expect("collect_values");
  assert_eq!(values, vec![19_u32, 20_u32]);
}

#[test]
fn source_as_input_stream_collects_values() {
  let values = Source::from_array([21_u32, 22_u32]).as_input_stream().expect("as_input_stream");
  assert_eq!(values, vec![21_u32, 22_u32]);
}

#[test]
fn source_as_java_stream_collects_values() {
  let values = Source::from_array([23_u32, 24_u32]).as_java_stream().expect("as_java_stream");
  assert_eq!(values, vec![23_u32, 24_u32]);
}

#[test]
fn source_as_output_stream_collects_values() {
  let values = Source::from_array([25_u32, 26_u32]).as_output_stream().expect("as_output_stream");
  assert_eq!(values, vec![25_u32, 26_u32]);
}

#[test]
fn source_from_path_emits_path_bytes() {
  let values = Source::from_path("ab").collect_values().expect("collect_values");
  assert_eq!(values, vec![b'a', b'b']);
}

#[test]
fn source_broadcast_rejects_zero_fan_out() {
  assert!(Source::single(1_u32).broadcast(0).is_err());
}

#[test]
fn source_balance_keeps_single_path_behavior() {
  let values = Source::single(5_u32).balance(1).expect("balance").collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_balance_rejects_zero_fan_out() {
  assert!(Source::single(1_u32).balance(0).is_err());
}

#[test]
fn source_merge_keeps_single_path_behavior() {
  let values = Source::single(5_u32).merge(1).expect("merge").collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_merge_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).merge(0).is_err());
}

#[test]
fn source_zip_wraps_value_when_single_path() {
  let values = Source::single(5_u32).zip(1).expect("zip").collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_zip_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).zip(0).is_err());
}

#[test]
fn source_concat_keeps_single_path_behavior() {
  let values = Source::single(5_u32).concat(1).expect("concat").collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_concat_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).concat(0).is_err());
}

#[test]
fn source_partition_keeps_single_path_behavior() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .partition(|value| value % 2 == 0)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

#[test]
fn source_unzip_emits_tuple_components() {
  let values = Source::single((5_u32, 6_u32)).unzip().collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_unzip_with_emits_mapped_tuple_components() {
  let values = Source::single(5_u32)
    .unzip_with(|value| (value, value.saturating_add(1)))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_interleave_keeps_single_path_behavior() {
  let values = Source::single(5_u32).interleave(1).expect("interleave").collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_interleave_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).interleave(0).is_err());
}

#[test]
fn source_prepend_keeps_single_path_behavior() {
  let values = Source::single(5_u32).prepend(1).expect("prepend").collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_prepend_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).prepend(0).is_err());
}

#[test]
fn source_zip_all_wraps_value_when_single_path() {
  let values = Source::single(5_u32).zip_all(1, 0_u32).expect("zip_all").collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_zip_all_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).zip_all(0, 0_u32).is_err());
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
fn source_flat_map_merge_preserves_outer_order_and_round_robin() {
  let values = Source::from_array([1_u32, 2_u32])
    .flat_map_merge(2, |value| Source::from_array([value, value.saturating_add(10)]))
    .expect("flat_map_merge")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 11_u32, 2_u32, 12_u32]);
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
fn source_flat_map_merge_skips_empty_inner_and_completes() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .flat_map_merge(
      2,
      |value| {
        if value == 1 { Source::empty() } else { Source::from_array([value.saturating_add(10)]) }
      },
    )
    .expect("flat_map_merge")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![12_u32, 13_u32]);
}

#[test]
fn source_flat_map_concat_keeps_order_with_empty_inner_stream() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32]).flat_map_concat(|value| {
    if value == 1 { Source::empty() } else { Source::from_array([value.saturating_add(20), value.saturating_add(30)]) }
  });
  let values = values.collect_values().expect("collect_values");
  assert_eq!(values, vec![22_u32, 32_u32, 23_u32, 33_u32]);
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
fn source_throttle_keeps_single_path_behavior() {
  let values = Source::single(5_u32)
    .throttle(2, crate::core::ThrottleMode::Shaping)
    .expect("throttle")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_throttle_rejects_zero_capacity() {
  let result = Source::single(1_u32).throttle(0, crate::core::ThrottleMode::Shaping);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_delay_keeps_single_path_behavior() {
  let values = Source::single(5_u32).delay(2).expect("delay").collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_delay_rejects_zero_ticks() {
  let result = Source::single(1_u32).delay(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_initial_delay_keeps_single_path_behavior() {
  let values = Source::single(5_u32).initial_delay(2).expect("initial_delay").collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_initial_delay_rejects_zero_ticks() {
  let result = Source::single(1_u32).initial_delay(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_take_within_limits_output_window() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .take_within(1)
    .expect("take_within")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn source_take_within_rejects_zero_ticks() {
  let result = Source::single(1_u32).take_within(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_batch_emits_fixed_size_chunks() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32, 4_u32, 5_u32])
    .batch(2)
    .expect("batch")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32], vec![5_u32]]);
}

#[test]
fn source_batch_rejects_zero_size() {
  let result = Source::single(1_u32).batch(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
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
fn source_p2_regression_group_by_merge_substreams_with_delay_and_zip_all() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .group_by(4, |value: &u32| value % 2)
    .expect("group_by")
    .merge_substreams()
    .delay(1)
    .expect("delay")
    .zip_all(1, 0_u32)
    .expect("zip_all")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32], vec![2_u32], vec![3_u32]]);
}

#[test]
fn source_p2_regression_concat_substreams_with_take_within_and_prepend() {
  let values = Source::single(vec![4_u32, 5_u32])
    .concat_substreams()
    .take_within(2)
    .expect("take_within")
    .prepend(1)
    .expect("prepend")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![4_u32, 5_u32]);
}

#[test]
fn source_map_error_maps_error_payload() {
  let values = Source::single(Err::<u32, StreamError>(StreamError::Failed))
    .map_error(|_| StreamError::WouldBlock)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![Err(StreamError::WouldBlock)]);
}

#[test]
fn source_on_error_continue_drops_error_payloads() {
  let values = Source::from_array([
    Ok::<u32, StreamError>(1_u32),
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(2_u32),
  ])
  .on_error_continue()
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_on_error_resume_alias_drops_error_payloads() {
  let values = Source::from_array([
    Ok::<u32, StreamError>(1_u32),
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(2_u32),
  ])
  .on_error_resume()
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_on_error_complete_stops_emitting_after_first_error_payload() {
  let values = Source::from_array([
    Ok::<u32, StreamError>(1_u32),
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(2_u32),
  ])
  .on_error_complete()
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
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
fn source_recover_preserves_ok_values_and_replaces_error_payloads() {
  let values = Source::from_array([
    Ok::<u32, StreamError>(1_u32),
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(2_u32),
  ])
  .recover(5_u32)
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 5_u32, 2_u32]);
}

#[test]
fn source_recover_with_alias_replaces_error_payload_with_fallback() {
  let values = Source::single(Err::<u32, StreamError>(StreamError::Failed))
    .recover_with(8_u32)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn source_recover_with_retries_fails_when_retry_budget_is_exhausted() {
  let result =
    Source::single(Err::<u32, StreamError>(StreamError::Failed)).recover_with_retries(0, 5_u32).collect_values();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn source_recover_with_retries_emits_fallback_until_budget_exhausts() {
  let values = Source::from_array([
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(5_u32),
    Err::<u32, StreamError>(StreamError::Failed),
  ])
  .recover_with_retries(2, 7_u32)
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![7_u32, 5_u32, 7_u32]);
}

#[test]
fn source_recover_with_retries_fails_after_consuming_retry_budget() {
  let result =
    Source::from_array([Err::<u32, StreamError>(StreamError::Failed), Err::<u32, StreamError>(StreamError::Failed)])
      .recover_with_retries(1, 7_u32)
      .collect_values();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn source_restart_with_backoff_keeps_single_path_behavior() {
  let values = Source::single(5_u32).restart_source_with_backoff(1, 3).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_on_failures_with_backoff_alias_keeps_single_path_behavior() {
  let values = Source::single(5_u32).on_failures_with_backoff(1, 3).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_with_backoff_alias_keeps_single_path_behavior() {
  let values = Source::single(5_u32).with_backoff(1, 3).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_with_backoff_and_context_alias_keeps_single_path_behavior() {
  let values = Source::single(5_u32).with_backoff_and_context(1, 3, "compat").collect_values().expect("collect_values");
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

#[test]
fn source_detach_preserves_elements_and_order() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .detach()
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_fold_emits_running_accumulation_without_initial() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .fold(0_u32, |acc, value| acc + value)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn source_reduce_folds_with_first_element_as_seed() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .reduce(|acc, value| acc + value)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn source_lazy_source_persists_error_on_collect_values_failure() {
  let mut logic = super::LazySourceLogic::<u32, _> {
    factory: Some(|| Source::<u32, StreamNotUsed>::failed(StreamError::Failed)),
    buffer:  VecDeque::new(),
    error:   None,
    _pd:     PhantomData,
  };

  // Given: 初回 pull で factory が消費され collect_values が失敗する
  let first = logic.pull();
  assert!(matches!(first, Err(StreamError::Failed)));

  // When: 後続 pull を呼ぶ（factory は既に消費済み）
  let second = logic.pull();
  // Then: 偽の正常完了（Ok(None)）ではなくエラーを返す
  assert!(matches!(second, Err(StreamError::Failed)));

  // When: on_restart を呼ぶ
  let restart = logic.on_restart();
  // Then: エラー状態が永続化されリスタートも失敗する
  assert!(matches!(restart, Err(StreamError::Failed)));
}
#[test]
fn source_distinct_removes_duplicate_elements() {
  let values = Source::from_array([3_u32, 1, 2, 1, 3, 2, 4]).distinct().collect_values().expect("collect_values");
  assert_eq!(values, vec![3_u32, 1, 2, 4]);
}

#[test]
fn source_distinct_on_already_unique_passes_all() {
  let values = Source::from_array([1_u32, 2, 3]).distinct().collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn source_distinct_by_removes_elements_with_duplicate_key() {
  let values = Source::from_array([(1_u32, "a"), (2, "b"), (1, "c"), (3, "d")])
    .distinct_by(|pair: &(u32, &str)| pair.0)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![(1_u32, "a"), (2, "b"), (3, "d")]);
}

#[test]
fn source_from_graph_creates_source_from_existing_graph() {
  let original = Source::from_array([10_u32, 20, 30]);
  let (graph, mat) = original.into_parts();
  let reconstructed = Source::<u32, StreamNotUsed>::from_graph(graph, mat);
  let values = reconstructed.collect_values().expect("collect_values");
  assert_eq!(values, vec![10_u32, 20, 30]);
}

#[test]
fn source_pre_materialize_returns_source_and_completion() {
  let source: Source<u32, StreamCompletion<StreamDone>> =
    Source::<u32, StreamNotUsed>::empty().map_materialized_value(|_| StreamCompletion::<StreamDone>::new());
  let (source, completion) = source.pre_materialize();
  let _ = source;
  assert!(completion.try_take().is_none());
}

#[test]
fn source_throttle_enforcing_mode_keeps_single_path() {
  let values = Source::single(5_u32)
    .throttle(2, crate::core::ThrottleMode::Enforcing)
    .expect("throttle")
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_throttle_enforcing_mode_fails_on_capacity_overflow() {
  let result = Source::single(alloc::vec![1_u32, 2, 3])
    .map_concat(|v: alloc::vec::Vec<u32>| v)
    .throttle(1, crate::core::ThrottleMode::Enforcing)
    .expect("throttle")
    .collect_values();
  assert_eq!(result, Err(StreamError::BufferOverflow));
}

#[test]
fn source_named_is_noop() {
  let values = Source::from_array([1_u32, 2, 3]).named("test-source").collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn source_from_materializer_creates_source() {
  let values = Source::from_materializer(|| Source::from_array([10_u32, 20])).collect_values().expect("collect_values");
  assert_eq!(values, vec![10_u32, 20]);
}

#[test]
fn source_debounce_rejects_zero_ticks() {
  let result = Source::from_array([1_u32]).debounce(0);
  assert!(result.is_err());
}

#[test]
fn source_sample_rejects_zero_ticks() {
  let result = Source::from_array([1_u32]).sample(0);
  assert!(result.is_err());
}

#[test]
fn source_debounce_keeps_single_path_behavior() {
  let values = Source::single(7_u32).debounce(1).expect("debounce").collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_sample_keeps_single_path_behavior() {
  let values = Source::single(7_u32).sample(1).expect("sample").collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}
