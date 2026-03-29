use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{future::Future, marker::PhantomData, pin::Pin, task::Poll};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::core::{
  DynValue, FlowLogic, SourceLogic, StageDefinition, StreamDone, StreamDslError, StreamError, StreamNotUsed,
  SubstreamCancelStrategy,
  buffer::{OverflowStrategy, StreamBufferConfig},
  dsl::{Flow, FlowMonitorImpl, Sink, Source, TailSource},
  lifecycle::{DriveOutcome, Stream},
  materialization::{Completion, KeepBoth, KeepLeft, KeepRight, StreamCompletion},
  operator::{DefaultOperatorCatalog, OperatorCatalog, OperatorKey},
  queue::QueueOfferResult,
  restart::RestartSettings,
  shape::UniformFanInShape,
  stage::StageKind,
};

#[cfg(feature = "compression")]
const FLOW_DECOMPRESSION_MAX_BYTES_DEFAULT: usize = 1024 * 1024;

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

struct CountingSequenceSourceLogic {
  values: VecDeque<u32>,
  pulls:  ArcShared<SpinSyncMutex<usize>>,
}

impl CountingSequenceSourceLogic {
  fn new(values: &[u32], pulls: ArcShared<SpinSyncMutex<usize>>) -> Self {
    let mut queue = VecDeque::with_capacity(values.len());
    queue.extend(values.iter().copied());
    Self { values: queue, pulls }
  }
}

impl SourceLogic for CountingSequenceSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    {
      let mut guard = self.pulls.lock();
      *guard = guard.saturating_add(1);
    }
    Ok(self.values.pop_front().map(|value| Box::new(value) as DynValue))
  }
}

struct PulsedSourceLogic {
  schedule: VecDeque<Option<u32>>,
}

impl PulsedSourceLogic {
  fn new(schedule: &[Option<u32>]) -> Self {
    let mut queue = VecDeque::with_capacity(schedule.len());
    queue.extend(schedule.iter().copied());
    Self { schedule: queue }
  }
}

impl SourceLogic for PulsedSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    let Some(next) = self.schedule.pop_front() else {
      return Ok(None);
    };
    match next {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Err(StreamError::WouldBlock),
    }
  }
}

struct FailureSequenceSourceLogic {
  steps: VecDeque<Result<u32, StreamError>>,
}

impl FailureSequenceSourceLogic {
  fn new(steps: &[Result<u32, StreamError>]) -> Self {
    let mut queue = VecDeque::with_capacity(steps.len());
    queue.extend(steps.iter().cloned());
    Self { steps: queue }
  }
}

struct NonCloneValue(u32);

impl SourceLogic for FailureSequenceSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.steps.pop_front() {
      | Some(Ok(value)) => Ok(Some(Box::new(value) as DynValue)),
      | Some(Err(error)) => Err(error),
      | None => Ok(None),
    }
  }
}

fn drive_until_completion<T>(stream: &mut Stream, completion: &StreamCompletion<T>)
where
  T: Clone, {
  let mut idle_budget = 1024_usize;
  let mut drive_budget = 16384_usize;
  while !matches!(completion.poll(), Completion::Ready(_)) {
    assert!(drive_budget > 0, "stream did not reach completion within drive budget");
    drive_budget = drive_budget.saturating_sub(1);
    match stream.drive() {
      | DriveOutcome::Progressed => idle_budget = 1024,
      | DriveOutcome::Idle => {
        assert!(idle_budget > 0, "stream stalled");
        idle_budget = idle_budget.saturating_sub(1);
      },
    }
  }
}

#[test]
fn broadcast_duplicates_each_element() {
  let values =
    Source::single(7_u32).via(Flow::new().broadcast(2).expect("broadcast")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32, 7_u32]);
}

#[test]
fn broadcast_rejects_zero_fan_out() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.broadcast(0).is_err());
}

#[test]
fn balance_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().balance(1).expect("balance")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn balance_rejects_zero_fan_out() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.balance(0).is_err());
}

#[test]
fn merge_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().merge(1).expect("merge")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.merge(0).is_err());
}

#[test]
fn zip_wraps_value_when_single_path() {
  let values = Source::single(7_u32).via(Flow::new().zip(1).expect("zip")).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn zip_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.zip(0).is_err());
}

#[test]
fn concat_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().concat(1).expect("concat")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn concat_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.concat(0).is_err());
}

#[test]
fn partition_keeps_single_path_behavior() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().partition(|value| *value % 2 == 0))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

#[test]
fn unzip_emits_tuple_components() {
  let values = Source::single((7_u32, 8_u32)).via(Flow::new().unzip()).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32, 8_u32]);
}

#[test]
fn unzip_with_emits_mapped_tuple_components() {
  let values = Source::single(7_u32)
    .via(Flow::new().unzip_with(|value: u32| (value, value.saturating_add(1))))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32, 8_u32]);
}

#[test]
fn interleave_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().interleave(1).expect("interleave")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn interleave_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.interleave(0).is_err());
}

#[test]
fn prepend_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().prepend(1).expect("prepend")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn prepend_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.prepend(0).is_err());
}

#[test]
fn concat_lazy_appends_secondary_source_after_primary_completion() {
  let values = Source::from_array([1_u32, 2])
    .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

#[test]
fn concat_lazy_emits_secondary_values_without_waiting_for_secondary_completion() {
  let (secondary_graph, mut secondary_queue) = Source::<u32, _>::queue_unbounded().into_parts();
  let secondary = Source::from_graph(secondary_graph, StreamNotUsed::new());
  assert_eq!(secondary_queue.offer(10_u32), QueueOfferResult::Enqueued);

  let graph = Source::single(1_u32).via(Flow::new().concat_lazy(secondary).drop(1));
  let (plan, completion) = graph.into_mat(Sink::head(), KeepRight).into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");

  drive_until_completion(&mut stream, &completion);

  assert_eq!(completion.poll(), Completion::Ready(Ok(10_u32)));
}

#[test]
fn prepend_lazy_emits_secondary_source_before_primary_values() {
  let values = Source::from_array([3_u32, 4])
    .via(Flow::new().prepend_lazy(Source::from_array([1_u32, 2])))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

#[test]
fn prepend_lazy_emits_secondary_values_without_waiting_for_secondary_completion() {
  let (secondary_graph, mut secondary_queue) = Source::<u32, _>::queue_unbounded().into_parts();
  let secondary = Source::from_graph(secondary_graph, StreamNotUsed::new());
  assert_eq!(secondary_queue.offer(1_u32), QueueOfferResult::Enqueued);

  let graph = Source::single(3_u32).via(Flow::new().prepend_lazy(secondary));
  let (plan, completion) = graph.into_mat(Sink::head(), KeepRight).into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");

  drive_until_completion(&mut stream, &completion);

  assert_eq!(completion.poll(), Completion::Ready(Ok(1_u32)));
}

#[test]
fn or_else_uses_secondary_source_only_when_primary_is_empty() {
  let values = Source::<u32, _>::empty()
    .via(Flow::new().or_else(Source::from_array([5_u32, 6])))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn or_else_ignores_secondary_source_after_primary_emits() {
  let values = Source::from_array([7_u32, 8])
    .via(Flow::new().or_else(Source::from_array([1_u32, 2])))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32, 8_u32]);
}

#[test]
fn or_else_emits_secondary_values_without_waiting_for_secondary_completion() {
  let (secondary_graph, mut secondary_queue) = Source::<u32, _>::queue_unbounded().into_parts();
  let secondary = Source::from_graph(secondary_graph, StreamNotUsed::new());
  assert_eq!(secondary_queue.offer(5_u32), QueueOfferResult::Enqueued);

  let graph = Source::<u32, _>::empty().via(Flow::new().or_else(secondary));
  let (plan, completion) = graph.into_mat(Sink::head(), KeepRight).into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");

  drive_until_completion(&mut stream, &completion);

  assert_eq!(completion.poll(), Completion::Ready(Ok(5_u32)));
}

#[test]
fn concat_lazy_materializes_secondary_after_primary_finishes() {
  let materialize_calls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let secondary = Source::lazy_source({
    let materialize_calls = materialize_calls.clone();
    move || {
      let mut guard = materialize_calls.lock();
      *guard = guard.saturating_add(1);
      Source::from_array([3_u32, 4_u32])
    }
  });
  assert_eq!(*materialize_calls.lock(), 0_u32);
  let primary = Source::<u32, _>::from_logic(StageKind::Custom, PulsedSourceLogic::new(&[Some(1_u32), None, None]));
  let values = primary.via(Flow::new().concat_lazy(secondary)).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 4_u32]);
  assert_eq!(*materialize_calls.lock(), 1_u32);
}

#[test]
fn prepend_lazy_materializes_secondary_on_first_demand() {
  let materialize_calls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let secondary = Source::lazy_source({
    let materialize_calls = materialize_calls.clone();
    move || {
      let mut guard = materialize_calls.lock();
      *guard = guard.saturating_add(1);
      Source::from_array([1_u32, 2_u32])
    }
  });
  let graph = Source::single(3_u32).via(Flow::new().prepend_lazy(secondary));
  let (plan, completion) = graph.into_mat(Sink::head(), KeepRight).into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  assert_eq!(*materialize_calls.lock(), 0_u32);

  stream.start().expect("start");
  drive_until_completion(&mut stream, &completion);

  assert_eq!(completion.poll(), Completion::Ready(Ok(1_u32)));
  assert_eq!(*materialize_calls.lock(), 1_u32);
}

#[test]
fn or_else_does_not_materialize_secondary_when_primary_emits() {
  let materialize_calls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let secondary = Source::lazy_source({
    let materialize_calls = materialize_calls.clone();
    move || {
      let mut guard = materialize_calls.lock();
      *guard = guard.saturating_add(1);
      Source::from_array([9_u32, 10_u32])
    }
  });
  let values =
    Source::from_array([7_u32, 8_u32]).via(Flow::new().or_else(secondary)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32, 8_u32]);
  assert_eq!(*materialize_calls.lock(), 0_u32);
}

#[test]
fn concat_lazy_accepts_non_clone_elements() {
  let values = Source::from_array([NonCloneValue(1_u32), NonCloneValue(2_u32)])
    .via(Flow::new().concat_lazy(Source::from_array([NonCloneValue(3_u32)])))
    .map(|value: NonCloneValue| value.0)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn prepend_lazy_accepts_non_clone_elements() {
  let values = Source::from_array([NonCloneValue(2_u32), NonCloneValue(3_u32)])
    .via(Flow::new().prepend_lazy(Source::from_array([NonCloneValue(1_u32)])))
    .map(|value: NonCloneValue| value.0)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn or_else_accepts_non_clone_elements() {
  let values = Source::from_array([NonCloneValue(7_u32)])
    .via(Flow::new().or_else(Source::from_array([NonCloneValue(9_u32)])))
    .map(|value: NonCloneValue| value.0)
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn concat_lazy_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 11_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 7_u32)
    .concat_lazy_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 7_u32);
  assert_eq!(right_mat, 11_u32);
}

#[test]
fn concat_lazy_mat_preserves_existing_data_path_behavior() {
  let values = Source::from_array([1_u32, 2_u32])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .concat_lazy_mat(Source::from_array([3_u32, 4_u32]).map_materialized_value(|_| 99_u32), KeepLeft),
    )
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

#[test]
fn prepend_lazy_mat_combines_materialized_values() {
  let secondary = Source::single(1_u32).map_materialized_value(|_| 13_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 5_u32)
    .prepend_lazy_mat(secondary, KeepRight)
    .into_parts();

  assert_eq!(materialized, 13_u32);
}

#[test]
fn prepend_lazy_mat_preserves_existing_data_path_behavior() {
  let values = Source::from_array([3_u32, 4_u32])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .prepend_lazy_mat(Source::from_array([1_u32, 2_u32]).map_materialized_value(|_| 42_u32), KeepLeft),
    )
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

#[test]
fn or_else_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 17_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 3_u32)
    .or_else_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(materialized, (3_u32, 17_u32));
}

#[test]
fn or_else_mat_preserves_existing_data_path_behavior() {
  let values = Source::<u32, _>::empty()
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .or_else_mat(Source::from_array([5_u32, 6_u32]).map_materialized_value(|_| 77_u32), KeepLeft),
    )
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn divert_to_mat_combines_materialized_values() {
  let sink = Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| 23_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 19_u32)
    .divert_to_mat(|value: &u32| (*value).is_multiple_of(2), sink, KeepRight)
    .into_parts();

  assert_eq!(materialized, 23_u32);
}

#[test]
fn divert_to_mat_preserves_existing_data_path_behavior() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32, 4_u32])
    .via(Flow::<u32, u32, StreamNotUsed>::new().divert_to_mat(
      |value: &u32| (*value).is_multiple_of(2),
      Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| 1_u32),
      KeepLeft,
    ))
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![1_u32, 3_u32]);
}

#[test]
fn divert_to_mat_routes_matching_elements_to_sink() {
  let diverted = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let diverted_ref = diverted.clone();

  let values = Source::from_array([1_u32, 2_u32, 3_u32, 4_u32])
    .via(Flow::<u32, u32, StreamNotUsed>::new().divert_to_mat(
      |value: &u32| (*value).is_multiple_of(2),
      Sink::<u32, StreamCompletion<StreamDone>>::foreach(move |value| {
        diverted_ref.lock().push(value);
      }),
      KeepLeft,
    ))
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![1_u32, 3_u32]);
  assert_eq!(*diverted.lock(), vec![2_u32, 4_u32]);
}

// ---------------------------------------------------------------------------
// concat_mat
// ---------------------------------------------------------------------------

#[test]
fn concat_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 11_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 7_u32)
    .concat_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 7_u32);
  assert_eq!(right_mat, 11_u32);
}

#[test]
fn concat_mat_keeps_left_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 11_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 7_u32)
    .concat_mat(secondary, KeepLeft)
    .into_parts();

  assert_eq!(materialized, 7_u32);
}

#[test]
fn concat_mat_keeps_right_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 11_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 7_u32)
    .concat_mat(secondary, KeepRight)
    .into_parts();

  assert_eq!(materialized, 11_u32);
}

#[test]
fn concat_mat_preserves_existing_data_path_behavior() {
  let values = Source::from_array([1_u32, 2_u32])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .concat_mat(Source::from_array([3_u32, 4_u32]).map_materialized_value(|_| 99_u32), KeepLeft),
    )
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

// ---------------------------------------------------------------------------
// prepend_mat
// ---------------------------------------------------------------------------

#[test]
fn prepend_mat_combines_materialized_values() {
  let secondary = Source::single(1_u32).map_materialized_value(|_| 13_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 5_u32)
    .prepend_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 5_u32);
  assert_eq!(right_mat, 13_u32);
}

#[test]
fn prepend_mat_keeps_right_materialized_value() {
  let secondary = Source::single(1_u32).map_materialized_value(|_| 13_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 5_u32)
    .prepend_mat(secondary, KeepRight)
    .into_parts();

  assert_eq!(materialized, 13_u32);
}

#[test]
fn prepend_mat_preserves_existing_data_path_behavior() {
  let values = Source::from_array([3_u32, 4_u32])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .prepend_mat(Source::from_array([1_u32, 2_u32]).map_materialized_value(|_| 42_u32), KeepLeft),
    )
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

// ---------------------------------------------------------------------------
// merge_mat
// ---------------------------------------------------------------------------

#[test]
fn merge_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 17_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 3_u32)
    .merge_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 3_u32);
  assert_eq!(right_mat, 17_u32);
}

#[test]
fn merge_mat_keeps_left_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 17_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 3_u32)
    .merge_mat(secondary, KeepLeft)
    .into_parts();

  assert_eq!(materialized, 3_u32);
}

#[test]
fn merge_mat_preserves_existing_data_path_behavior() {
  let mut values = Source::single(7_u32)
    .map_materialized_value(|_| 0_u32)
    .merge_mat(Source::single(8_u32).map_materialized_value(|_| 99_u32), KeepLeft)
    .collect_values()
    .expect("collect_values");
  values.sort();

  assert!(values.contains(&7_u32));
  assert!(values.contains(&8_u32));
  assert_eq!(values.len(), 2);
}

// ---------------------------------------------------------------------------
// merge_preferred_mat
// ---------------------------------------------------------------------------

#[test]
fn merge_preferred_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 23_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 19_u32)
    .merge_preferred_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 19_u32);
  assert_eq!(right_mat, 23_u32);
}

#[test]
fn merge_preferred_mat_keeps_right_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 23_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 19_u32)
    .merge_preferred_mat(secondary, KeepRight)
    .into_parts();

  assert_eq!(materialized, 23_u32);
}

#[test]
fn merge_preferred_mat_preserves_existing_data_path_behavior() {
  let mut values = Source::single(7_u32)
    .map_materialized_value(|_| 0_u32)
    .merge_preferred_mat(Source::single(8_u32).map_materialized_value(|_| 99_u32), KeepLeft)
    .collect_values()
    .expect("collect_values");
  values.sort();

  assert!(values.contains(&7_u32));
  assert!(values.contains(&8_u32));
  assert_eq!(values.len(), 2);
}

// ---------------------------------------------------------------------------
// merge_sorted_mat
// ---------------------------------------------------------------------------

#[test]
fn merge_sorted_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 29_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 31_u32)
    .merge_sorted_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 31_u32);
  assert_eq!(right_mat, 29_u32);
}

#[test]
fn merge_sorted_mat_keeps_left_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 29_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 31_u32)
    .merge_sorted_mat(secondary, KeepLeft)
    .into_parts();

  assert_eq!(materialized, 31_u32);
}

#[test]
fn merge_sorted_mat_preserves_existing_data_path_behavior() {
  let values = Source::from_array([1_u32, 3_u32, 5_u32])
    .map_materialized_value(|_| 0_u32)
    .merge_sorted_mat(Source::from_array([2_u32, 4_u32, 6_u32]).map_materialized_value(|_| 99_u32), KeepLeft)
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32, 5_u32, 6_u32]);
}

// ---------------------------------------------------------------------------
// zip_mat
// ---------------------------------------------------------------------------

#[test]
fn zip_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 41_u32);

  let (_graph, (left_mat, right_mat)) =
    Flow::<u32, u32, StreamNotUsed>::new().map_materialized_value(|_| 37_u32).zip_mat(secondary, KeepBoth).into_parts();

  assert_eq!(left_mat, 37_u32);
  assert_eq!(right_mat, 41_u32);
}

#[test]
fn zip_mat_keeps_left_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 41_u32);

  let (_graph, materialized) =
    Flow::<u32, u32, StreamNotUsed>::new().map_materialized_value(|_| 37_u32).zip_mat(secondary, KeepLeft).into_parts();

  assert_eq!(materialized, 37_u32);
}

#[test]
fn zip_mat_preserves_existing_data_path_behavior() {
  let values = Source::single(1_u32)
    .map_materialized_value(|_| 0_u32)
    .zip_mat(Source::single(2_u32).map_materialized_value(|_| 99_u32), KeepLeft)
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![vec![1_u32, 2_u32]]);
}

// ---------------------------------------------------------------------------
// zip_all_mat
// ---------------------------------------------------------------------------

#[test]
fn zip_all_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 43_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 39_u32)
    .zip_all_mat(secondary, 0_u32, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 39_u32);
  assert_eq!(right_mat, 43_u32);
}

#[test]
fn zip_all_mat_keeps_right_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 43_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 39_u32)
    .zip_all_mat(secondary, 0_u32, KeepRight)
    .into_parts();

  assert_eq!(materialized, 43_u32);
}

#[test]
fn zip_all_mat_preserves_existing_data_path_behavior() {
  let values = Source::from_array([1_u32, 2_u32])
    .map_materialized_value(|_| 0_u32)
    .zip_all_mat(Source::from_array([3_u32]).map_materialized_value(|_| 99_u32), 0_u32, KeepLeft)
    .collect_values()
    .expect("collect_values");

  // zip_all pads shorter stream with fill_value (0)
  assert_eq!(values, vec![vec![1_u32, 3_u32], vec![2_u32, 0_u32]]);
}

// ---------------------------------------------------------------------------
// zip_with_mat
// ---------------------------------------------------------------------------

#[test]
fn zip_with_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 47_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 45_u32)
    .zip_with_mat(secondary, |values: Vec<u32>| values.into_iter().sum::<u32>(), KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 45_u32);
  assert_eq!(right_mat, 47_u32);
}

#[test]
fn zip_with_mat_keeps_right_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 47_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 45_u32)
    .zip_with_mat(secondary, |values: Vec<u32>| values.into_iter().sum::<u32>(), KeepRight)
    .into_parts();

  assert_eq!(materialized, 47_u32);
}

#[test]
fn zip_with_mat_preserves_existing_data_path_behavior() {
  let values = Source::single(10_u32)
    .map_materialized_value(|_| 0_u32)
    .zip_with_mat(
      Source::single(20_u32).map_materialized_value(|_| 99_u32),
      |values: Vec<u32>| values.into_iter().sum::<u32>(),
      KeepLeft,
    )
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![30_u32]);
}

// ---------------------------------------------------------------------------
// zip_latest_mat
// ---------------------------------------------------------------------------

#[test]
fn zip_latest_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 51_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 49_u32)
    .zip_latest_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 49_u32);
  assert_eq!(right_mat, 51_u32);
}

#[test]
fn zip_latest_mat_keeps_left_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 51_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 49_u32)
    .zip_latest_mat(secondary, KeepLeft)
    .into_parts();

  assert_eq!(materialized, 49_u32);
}

#[test]
fn zip_latest_mat_preserves_existing_data_path_behavior() {
  let values = Source::single(1_u32)
    .map_materialized_value(|_| 0_u32)
    .zip_latest_mat(Source::single(2_u32).map_materialized_value(|_| 99_u32), KeepLeft)
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![vec![1_u32, 2_u32]]);
}

// ---------------------------------------------------------------------------
// zip_latest_with_mat
// ---------------------------------------------------------------------------

#[test]
fn zip_latest_with_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 55_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 53_u32)
    .zip_latest_with_mat(secondary, |values: Vec<u32>| values.into_iter().sum::<u32>(), KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 53_u32);
  assert_eq!(right_mat, 55_u32);
}

#[test]
fn zip_latest_with_mat_keeps_right_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 55_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 53_u32)
    .zip_latest_with_mat(secondary, |values: Vec<u32>| values.into_iter().sum::<u32>(), KeepRight)
    .into_parts();

  assert_eq!(materialized, 55_u32);
}

#[test]
fn zip_latest_with_mat_preserves_existing_data_path_behavior() {
  let values = Source::single(10_u32)
    .map_materialized_value(|_| 0_u32)
    .zip_latest_with_mat(
      Source::single(20_u32).map_materialized_value(|_| 99_u32),
      |values: Vec<u32>| values.into_iter().sum::<u32>(),
      KeepLeft,
    )
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![30_u32]);
}

// ---------------------------------------------------------------------------
// merge_latest_mat
// ---------------------------------------------------------------------------

#[test]
fn merge_latest_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 59_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 57_u32)
    .merge_latest_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 57_u32);
  assert_eq!(right_mat, 59_u32);
}

#[test]
fn merge_latest_mat_keeps_left_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 59_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 57_u32)
    .merge_latest_mat(secondary, KeepLeft)
    .into_parts();

  assert_eq!(materialized, 57_u32);
}

#[test]
fn merge_latest_mat_preserves_existing_data_path_behavior() {
  let values = Source::single(7_u32)
    .map_materialized_value(|_| 0_u32)
    .merge_latest_mat(Source::single(8_u32).map_materialized_value(|_| 99_u32), KeepLeft)
    .collect_values()
    .expect("collect_values");

  // merge_latest emits Vec of latest values from all inputs
  assert!(!values.is_empty());
}

// ---------------------------------------------------------------------------
// merge_prioritized_mat
// ---------------------------------------------------------------------------

#[test]
fn merge_prioritized_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 63_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 61_u32)
    .merge_prioritized_mat(secondary, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 61_u32);
  assert_eq!(right_mat, 63_u32);
}

#[test]
fn merge_prioritized_mat_keeps_right_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 63_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 61_u32)
    .merge_prioritized_mat(secondary, KeepRight)
    .into_parts();

  assert_eq!(materialized, 63_u32);
}

#[test]
fn merge_prioritized_mat_preserves_existing_data_path_behavior() {
  let mut values = Source::single(7_u32)
    .map_materialized_value(|_| 0_u32)
    .merge_prioritized_mat(Source::single(8_u32).map_materialized_value(|_| 99_u32), KeepLeft)
    .collect_values()
    .expect("collect_values");
  values.sort();

  assert!(values.contains(&7_u32));
  assert!(values.contains(&8_u32));
  assert_eq!(values.len(), 2);
}

// ---------------------------------------------------------------------------
// interleave_mat
// ---------------------------------------------------------------------------

#[test]
fn interleave_mat_combines_materialized_values() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 67_u32);

  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 65_u32)
    .interleave_mat(secondary, 1, KeepBoth)
    .into_parts();

  assert_eq!(left_mat, 65_u32);
  assert_eq!(right_mat, 67_u32);
}

#[test]
fn interleave_mat_keeps_left_materialized_value() {
  let secondary = Source::single(9_u32).map_materialized_value(|_| 67_u32);

  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 65_u32)
    .interleave_mat(secondary, 1, KeepLeft)
    .into_parts();

  assert_eq!(materialized, 65_u32);
}

#[test]
fn interleave_mat_preserves_existing_data_path_behavior() {
  let mut values = Source::from_array([1_u32, 3_u32])
    .map_materialized_value(|_| 0_u32)
    .interleave_mat(Source::from_array([2_u32, 4_u32]).map_materialized_value(|_| 99_u32), 1, KeepLeft)
    .collect_values()
    .expect("collect_values");
  values.sort();

  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

// ---------------------------------------------------------------------------
// flat_map_prefix_mat
// ---------------------------------------------------------------------------

#[test]
fn flat_map_prefix_mat_combines_materialized_values() {
  let (_graph, (left_mat, right_mat)) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 69_u32)
    .flat_map_prefix_mat(
      1,
      |_prefix: Vec<u32>| Flow::<u32, u32, StreamNotUsed>::new().map_materialized_value(|_| 71_u32),
      KeepBoth,
    )
    .into_parts();

  assert_eq!(left_mat, 69_u32);
  assert_eq!(right_mat, 71_u32);
}

#[test]
fn flat_map_prefix_mat_keeps_left_materialized_value() {
  let (_graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new()
    .map_materialized_value(|_| 69_u32)
    .flat_map_prefix_mat(
      1,
      |_prefix: Vec<u32>| Flow::<u32, u32, StreamNotUsed>::new().map_materialized_value(|_| 71_u32),
      KeepLeft,
    )
    .into_parts();

  assert_eq!(materialized, 69_u32);
}

#[test]
fn flat_map_prefix_mat_preserves_existing_data_path_behavior() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .via(Flow::<u32, u32, StreamNotUsed>::new().flat_map_prefix_mat(
      1,
      |_prefix: Vec<u32>| Flow::<u32, u32, StreamNotUsed>::new(),
      KeepLeft,
    ))
    .collect_values()
    .expect("collect_values");

  // flat_map_prefix consumes prefix (1 element), then passes rest through the inner flow
  assert_eq!(values, vec![2_u32, 3_u32]);
}

#[test]
fn zip_all_wraps_value_when_single_path() {
  let values = Source::single(7_u32)
    .via(Flow::new().zip_all(1, 0_u32).expect("zip_all"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn zip_all_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.zip_all(0, 0_u32).is_err());
}

#[test]
fn zip_latest_wraps_single_path_value_into_vec() {
  let values =
    Source::single(7_u32).via(Flow::new().zip_latest(1).expect("zip_latest")).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn zip_latest_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.zip_latest(0).is_err());
}

#[test]
fn zip_latest_with_maps_latest_snapshot() {
  let values = Source::single(7_u32)
    .via(Flow::new().zip_latest_with(1, |latest: Vec<u32>| latest[0].saturating_add(1)).expect("zip_latest_with"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn materialize_into_source_emits_completed_sink_materialized_value() {
  let (graph, completion) = Flow::new()
    .map(|value: u32| value.saturating_add(1))
    .materialize_into_source(Source::single(1_u32), Sink::head())
    .into_parts();
  let materialized: Vec<u32> =
    Source::from_graph(graph, StreamNotUsed::new()).collect_values().expect("collect_values");
  assert_eq!(materialized, vec![2_u32]);
  assert_eq!(completion.poll(), Completion::Ready(Ok(())));
}

#[test]
fn flat_map_merge_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().flat_map_merge(2, Source::single).expect("flat_map_merge"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn flat_map_concat_emits_head_without_waiting_for_inner_completion() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let graph = Source::single(0_u32)
    .via(Flow::new().flat_map_concat({
      let pulls = pulls.clone();
      move |_| {
        Source::<u32, _>::from_logic(StageKind::Custom, CountingSequenceSourceLogic::new(&[42, 43, 44], pulls.clone()))
      }
    }))
    .into_mat(Sink::head(), KeepRight);

  let (plan, completion) = graph.into_parts();
  let mut interpreter = Stream::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  drive_until_completion(&mut interpreter, &completion);

  assert_eq!(completion.poll(), Completion::Ready(Ok(42_u32)));
  assert_eq!(*pulls.lock(), 1_usize);
}

#[test]
fn flat_map_merge_emits_head_without_waiting_for_inner_completion() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let graph = Source::single(0_u32)
    .via(
      Flow::new()
        .flat_map_merge(1, {
          let pulls = pulls.clone();
          move |_| {
            Source::<u32, _>::from_logic(
              StageKind::Custom,
              CountingSequenceSourceLogic::new(&[42, 43, 44], pulls.clone()),
            )
          }
        })
        .expect("flat_map_merge"),
    )
    .into_mat(Sink::head(), KeepRight);

  let (plan, completion) = graph.into_parts();
  let mut interpreter = Stream::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  drive_until_completion(&mut interpreter, &completion);

  assert_eq!(completion.poll(), Completion::Ready(Ok(42_u32)));
  assert_eq!(*pulls.lock(), 1_usize);
}

#[test]
fn flat_map_merge_rejects_zero_breadth() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.flat_map_merge(0, Source::single);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "breadth", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn buffer_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().buffer(2, OverflowStrategy::Backpressure).expect("buffer"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn buffer_rejects_zero_capacity() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.buffer(0, OverflowStrategy::Backpressure);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
#[allow(deprecated)]
fn async_boundary_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().async_boundary()).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn throttle_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().throttle(2, crate::core::ThrottleMode::Shaping).expect("throttle"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn throttle_rejects_zero_capacity() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.throttle(0, crate::core::ThrottleMode::Shaping);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn delay_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().delay(2).expect("delay")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn delay_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.delay(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn initial_delay_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().initial_delay(2).expect("initial_delay"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn initial_delay_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.initial_delay(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn take_within_limits_output_window() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().take_within(1).expect("take_within"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn take_within_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.take_within(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn batch_emits_fixed_size_chunks() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]))
    .via(Flow::new().batch(2).expect("batch"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32], vec![5_u32]]);
}

#[test]
fn batch_rejects_zero_size() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.batch(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn map_async_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().map_async(2, |value: u32| async move { value.saturating_add(1) }).expect("map_async"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn map_async_rejects_zero_parallelism() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.map_async(0, |value| async move { value });
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "parallelism", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn map_async_preserves_order_with_parallelism() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .via(
      Flow::new()
        .map_async(2, |value: u32| match value {
          | 1 => YieldThenOutputFuture::new_with_poll_count(value.saturating_add(1), 2),
          | 2 => YieldThenOutputFuture::new(value.saturating_add(1)),
          | _ => YieldThenOutputFuture::new_with_poll_count(value.saturating_add(1), 3),
        })
        .expect("map_async"),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 3_u32, 4_u32]);
}

#[test]
fn map_async_logic_keeps_order_and_tracks_pending_output() {
  let mut logic = super::MapAsyncLogic::<u32, u32, _, YieldThenOutputFuture<u32>> {
    func:        |value: u32| YieldThenOutputFuture::new(value.saturating_add(1)),
    parallelism: 2,
    pending:     VecDeque::new(),
    _pd:         core::marker::PhantomData,
  };

  assert!(logic.can_accept_input());
  let _ = logic.apply(Box::new(1_u32)).expect("apply");
  assert!(logic.has_pending_output());

  assert!(logic.can_accept_input());
  let _ = logic.apply(Box::new(2_u32)).expect("apply");
  assert!(!logic.can_accept_input());

  let outputs = logic.drain_pending().expect("drain");
  assert_eq!(outputs.len(), 0);

  let outputs = logic.drain_pending().expect("drain");
  assert_eq!(outputs.len(), 2);
  let output_values: Vec<u32> =
    outputs.into_iter().map(|value: DynValue| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(output_values, vec![2_u32, 3_u32]);
  assert!(!logic.has_pending_output());
  assert!(logic.can_accept_input());
}

#[test]
fn conflate_with_seed_logic_defers_and_merges_pending_values() {
  let mut logic = super::ConflateWithSeedLogic::<u32, u32, _, _> {
    seed:         |value| value + 10,
    aggregate:    |acc, value| acc + value,
    pending:      None,
    just_updated: false,
    _pd:          core::marker::PhantomData,
  };

  assert!(logic.can_accept_input());
  let first = logic.apply(Box::new(1_u32)).expect("first apply");
  assert!(first.is_empty());
  assert!(logic.can_accept_input());
  assert!(logic.drain_pending().expect("first deferred drain").is_empty());

  let second = logic.apply(Box::new(2_u32)).expect("second apply");
  assert!(second.is_empty());
  assert!(logic.can_accept_input());
  assert!(logic.drain_pending().expect("second deferred drain").is_empty());

  let flushed = logic.drain_pending().expect("flush pending");
  let flushed_values: Vec<u32> = flushed.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(flushed_values, vec![13_u32]);

  let third = logic.apply(Box::new(3_u32)).expect("third apply");
  assert!(third.is_empty());
  assert!(logic.can_accept_input());
  assert!(logic.drain_pending().expect("third deferred drain").is_empty());
  let flushed_third = logic.drain_pending().expect("flush third");
  let flushed_third_values: Vec<u32> =
    flushed_third.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(flushed_third_values, vec![13_u32]);
}

#[derive(Default)]
struct YieldThenOutputFuture<T> {
  value:       Option<T>,
  poll_count:  u8,
  ready_after: u8,
}

impl<T> YieldThenOutputFuture<T> {
  fn new(value: T) -> Self {
    Self { value: Some(value), poll_count: 0, ready_after: 1 }
  }

  fn new_with_poll_count(value: T, poll_count: u8) -> Self {
    Self { value: Some(value), poll_count: 0, ready_after: poll_count }
  }
}

impl<T: Unpin> Future for YieldThenOutputFuture<T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, _cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    if this.poll_count < this.ready_after {
      this.poll_count = this.poll_count.saturating_add(1);
      Poll::Pending
    } else {
      Poll::Ready(this.value.take().expect("future value"))
    }
  }
}

struct PartitionedYieldFuture {
  value:         Option<u32>,
  partition:     usize,
  poll_count:    u8,
  ready_after:   u8,
  active_counts:
    fraktor_utils_rs::core::sync::ArcShared<fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex<[u32; 2]>>,
}

impl PartitionedYieldFuture {
  fn new(
    value: u32,
    partition: usize,
    ready_after: u8,
    active_counts: fraktor_utils_rs::core::sync::ArcShared<
      fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex<[u32; 2]>,
    >,
  ) -> Self {
    Self::new_with_overlap(value, partition, ready_after, active_counts, None)
  }

  fn new_with_overlap(
    value: u32,
    partition: usize,
    ready_after: u8,
    active_counts: fraktor_utils_rs::core::sync::ArcShared<
      fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex<[u32; 2]>,
    >,
    overlap_seen: Option<
      &fraktor_utils_rs::core::sync::ArcShared<fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex<bool>>,
    >,
  ) -> Self {
    {
      let mut guard = active_counts.lock();
      assert_eq!(guard[partition], 0, "same partition started concurrently");
      guard[partition] = guard[partition].saturating_add(1);
      if guard.iter().copied().sum::<u32>() > 1
        && let Some(overlap_seen) = overlap_seen
      {
        *overlap_seen.lock() = true;
      }
    }
    Self { value: Some(value), partition, poll_count: 0, ready_after, active_counts }
  }
}

impl Future for PartitionedYieldFuture {
  type Output = u32;

  fn poll(self: Pin<&mut Self>, _cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
    // Safety: pin 済みのフィールドを move しないため安全。
    let this = unsafe { self.get_unchecked_mut() };
    if this.poll_count < this.ready_after {
      this.poll_count = this.poll_count.saturating_add(1);
      return Poll::Pending;
    }

    {
      let mut guard = this.active_counts.lock();
      guard[this.partition] = guard[this.partition].saturating_sub(1);
    }
    Poll::Ready(this.value.take().expect("partitioned future value"))
  }
}

#[test]
fn map_async_partitioned_serializes_same_partition_while_preserving_input_order() {
  let active_counts = fraktor_utils_rs::core::sync::ArcShared::new(
    fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex::new([0_u32; 2]),
  );
  let values = Source::from_array([1_u32, 3, 2, 4])
    .via(
      Flow::new()
        .map_async_partitioned(2, |value: &u32| (*value as usize) % 2, {
          let active_counts = active_counts.clone();
          move |value: u32, partition: usize| {
            let ready_after = if partition == 1 { 2 } else { 1 };
            PartitionedYieldFuture::new(value.saturating_add(10), partition, ready_after, active_counts.clone())
          }
        })
        .expect("map_async_partitioned"),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![11_u32, 13_u32, 12_u32, 14_u32]);
}

#[test]
fn map_async_partitioned_unordered_emits_completed_partitions_without_global_ordering() {
  let active_counts = fraktor_utils_rs::core::sync::ArcShared::new(
    fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex::new([0_u32; 2]),
  );
  let overlap_seen = fraktor_utils_rs::core::sync::ArcShared::new(
    fraktor_utils_rs::core::sync::sync_mutex_like::SpinSyncMutex::new(false),
  );
  let values = Source::from_array([1_u32, 2_u32])
    .via(
      Flow::new()
        .map_async_partitioned_unordered(2, |value: &u32| (*value as usize) % 2, {
          let active_counts = active_counts.clone();
          let overlap_seen = overlap_seen.clone();
          move |value: u32, partition: usize| {
            let ready_after = if partition == 1 { 16 } else { 0 };
            PartitionedYieldFuture::new_with_overlap(
              value.saturating_add(10),
              partition,
              ready_after,
              active_counts.clone(),
              Some(&overlap_seen),
            )
          }
        })
        .expect("map_async_partitioned_unordered"),
    )
    .collect_values()
    .expect("collect_values");
  assert!(*overlap_seen.lock(), "different partitions should overlap in flight");
  assert_eq!(values, vec![12_u32, 11_u32]);
}

#[test]
fn filter_keeps_matching_elements() {
  let values =
    Source::single(7_u32).via(Flow::new().filter(|value| *value % 2 == 1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn filter_discards_non_matching_elements() {
  let values =
    Source::single(8_u32).via(Flow::new().filter(|value| *value % 2 == 1)).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn filter_not_discards_matching_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().filter_not(|value| *value % 2 == 0))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32]);
}

#[test]
fn flatten_optional_emits_present_value() {
  let values =
    Source::single(Some(7_u32)).via(Flow::new().flatten_optional()).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn flatten_optional_skips_none() {
  let values =
    Source::single(None::<u32>).via(Flow::new().flatten_optional()).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn collect_maps_present_values_and_skips_absent_values() {
  let values = Source::from_array([1_i32, -1_i32, 2_i32])
    .via(Flow::new().collect(|value| u32::try_from(value).ok()))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn flatten_flattens_nested_sources_in_order_and_skips_empty_inner_sources() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .via(
      Flow::new()
        .map(|value: u32| {
          if value == 1 {
            Source::empty()
          } else {
            Source::from_array([value.saturating_add(20), value.saturating_add(30)])
          }
        })
        .flatten(),
    )
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![22_u32, 32_u32, 23_u32, 33_u32]);
}

#[test]
fn flatten_emits_inner_head_without_waiting_for_inner_completion() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let graph = Source::single(0_u32)
    .via(
      Flow::new()
        .map({
          let pulls = pulls.clone();
          move |_| {
            Source::<u32, _>::from_logic(
              StageKind::Custom,
              CountingSequenceSourceLogic::new(&[42, 43, 44], pulls.clone()),
            )
          }
        })
        .flatten(),
    )
    .into_mat(Sink::head(), KeepRight);

  let (plan, completion) = graph.into_parts();
  let mut interpreter = Stream::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  drive_until_completion(&mut interpreter, &completion);

  assert_eq!(completion.poll(), Completion::Ready(Ok(42_u32)));
  assert_eq!(*pulls.lock(), 1_usize);
}

#[test]
fn map_concat_expands_each_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().map_concat(|value: u32| [value, value.saturating_add(10)]))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 11_u32, 2_u32, 12_u32, 3_u32, 13_u32]);
}

#[test]
fn map_option_emits_only_present_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().map_option(|value| if value % 2 == 0 { Some(value) } else { None }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 4_u32]);
}

#[test]
fn stateful_map_emits_stateful_results() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().stateful_map(|| {
      let mut sum = 0_u32;
      move |value| {
        sum = sum.saturating_add(value);
        sum
      }
    }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn stateful_map_concat_expands_with_stateful_mapper() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().stateful_map_concat(|| {
      let mut sum = 0_u32;
      move |value| {
        sum = sum.saturating_add(value);
        [sum, sum.saturating_add(100)]
      }
    }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 101_u32, 3_u32, 103_u32, 6_u32, 106_u32]);
}

#[test]
fn stateful_map_on_complete_emits_final_element() {
  // 準備: on_complete で蓄積した合計値を末尾に出力する stateful_map
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().stateful_map_with_on_complete(
      || 0_u32,
      |state, value| {
        *state = state.saturating_add(value);
        value
      },
      |state| Some(state),
    ))
    .collect_values()
    .expect("collect_values");

  // 検証: 通常要素に加え、on_complete が出力した合計値が末尾に追加される
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 6_u32]);
}

#[test]
fn stateful_map_on_complete_none_emits_nothing_extra() {
  // 準備: on_complete が None を返す（末尾要素なし）
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().stateful_map_with_on_complete(
      || 0_u32,
      |state, value| {
        *state = state.saturating_add(value);
        value
      },
      |_state| None,
    ))
    .collect_values()
    .expect("collect_values");

  // 検証: on_complete が None を返したため、通常要素のみ
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn stateful_map_on_complete_receives_accumulated_state() {
  // 準備: state に値を蓄積し、on_complete で蓄積した合計を出力
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[10, 20, 30]))
    .via(Flow::new().stateful_map_with_on_complete(
      || 0_u32,
      |state, value| {
        *state = state.saturating_add(value);
        value
      },
      |state| Some(state),
    ))
    .collect_values()
    .expect("collect_values");

  // 検証: 各要素(10,20,30) + on_complete での蓄積合計(60)
  assert_eq!(values, vec![10_u32, 20, 30, 60]);
}

#[test]
fn stateful_map_concat_with_accumulator_processes_elements() {
  // 準備: StatefulMapConcatAccumulator を使用した stateful_map_concat
  use crate::core::dsl::StatefulMapConcatAccumulator;

  struct DoublingAccumulator;

  impl StatefulMapConcatAccumulator<u32, u32> for DoublingAccumulator {
    fn apply(&mut self, input: u32) -> alloc::vec::Vec<u32> {
      vec![input, input * 2]
    }
  }

  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().stateful_map_concat_with_accumulator(|| DoublingAccumulator))
    .collect_values()
    .expect("collect_values");

  // 検証: 各要素が [value, value*2] に展開される
  assert_eq!(values, vec![1_u32, 2, 2, 4, 3, 6]);
}

#[test]
fn stateful_map_concat_with_accumulator_on_complete_emits_trailing() {
  // 準備: on_complete で残りのバッファを排出する accumulator
  use crate::core::dsl::StatefulMapConcatAccumulator;

  struct BufferingAccumulator {
    buffer: alloc::vec::Vec<u32>,
  }

  impl StatefulMapConcatAccumulator<u32, u32> for BufferingAccumulator {
    fn apply(&mut self, input: u32) -> alloc::vec::Vec<u32> {
      self.buffer.push(input);
      // バッファが2つ溜まったら排出
      if self.buffer.len() >= 2 { core::mem::take(&mut self.buffer) } else { vec![] }
    }

    fn on_complete(&mut self) -> alloc::vec::Vec<u32> {
      // 残りのバッファを排出
      core::mem::take(&mut self.buffer)
    }
  }

  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().stateful_map_concat_with_accumulator(|| BufferingAccumulator { buffer: alloc::vec::Vec::new() }))
    .collect_values()
    .expect("collect_values");

  // 検証: [1,2] はバッファ満了で排出、[3] は on_complete で排出
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn drop_skips_first_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().drop(2))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn take_limits_emitted_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().take(2))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn drop_while_skips_matching_prefix() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().drop_while(|value| *value < 3))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn take_while_keeps_matching_prefix() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().take_while(|value| *value < 3))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn take_until_includes_first_matching_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().take_until(|value| *value >= 3))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn grouped_emits_fixed_size_chunks() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]))
    .via(Flow::new().grouped(2).expect("grouped"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32], vec![5_u32]]);
}

#[test]
fn grouped_rejects_zero_size() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.grouped(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn sliding_emits_overlapping_windows() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().sliding(3).expect("sliding"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32, 3_u32], vec![2_u32, 3_u32, 4_u32]]);
}

#[test]
fn sliding_rejects_zero_size() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.sliding(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn scan_emits_initial_and_running_accumulation() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().scan(0_u32, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![0_u32, 1_u32, 3_u32, 6_u32]);
}

#[test]
fn intersperse_injects_markers() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().intersperse(10_u32, 99_u32, 11_u32))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32, 1_u32, 99_u32, 2_u32, 99_u32, 3_u32, 11_u32]);
}

#[test]
fn intersperse_on_empty_stream_emits_start_and_end() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().intersperse(10_u32, 99_u32, 11_u32))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32, 11_u32]);
}

#[test]
fn zip_with_index_pairs_each_element_with_index() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[7, 8, 9]))
    .via(Flow::new().zip_with_index())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![(7_u32, 0_u64), (8_u32, 1_u64), (9_u32, 2_u64)]);
}

#[test]
fn group_by_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(
      Flow::new()
        .group_by(4, |value: &u32| value % 2, SubstreamCancelStrategy::default())
        .expect("group_by")
        .merge_substreams(),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn group_by_cancels_upstream_after_head_completion_by_default() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let graph =
    Source::<u32, _>::from_logic(StageKind::Custom, CountingSequenceSourceLogic::new(&[1, 2, 3], pulls.clone()))
      .via(
        Flow::new()
          .group_by(4, |value: &u32| value % 2, SubstreamCancelStrategy::default())
          .expect("group_by")
          .merge_substreams(),
      )
      .into_mat(Sink::head(), KeepRight);

  let (plan, completion) = graph.into_parts();
  let mut interpreter = Stream::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  drive_until_completion(&mut interpreter, &completion);

  assert_eq!(completion.poll(), Completion::Ready(Ok(1_u32)));
  assert_eq!(*pulls.lock(), 1_usize);
}

#[test]
fn group_by_rejects_zero_max_substreams() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.group_by(0, |value: &u32| *value, SubstreamCancelStrategy::default());
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "max_substreams", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn split_when_with_cancel_strategy_emits_single_segment_for_single_element() {
  let values = Source::single(7_u32)
    .via(Flow::new().split_when_with_cancel_strategy(SubstreamCancelStrategy::Drain, |_| false).into_flow())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn split_after_with_cancel_strategy_emits_single_segment_for_single_element() {
  let values = Source::single(7_u32)
    .via(Flow::new().split_after_with_cancel_strategy(SubstreamCancelStrategy::Propagate, |_| false).into_flow())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn split_when_emits_single_segment_for_single_element() {
  let values = Source::single(7_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_when(|_| false).into_flow())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn split_after_emits_single_segment_for_single_element() {
  let values = Source::single(7_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| false).into_flow())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn merge_substreams_flattens_single_segment() {
  let values = Source::single(7_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| true).merge_substreams())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn concat_substreams_flattens_single_segment() {
  let values = Source::single(7_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| true).concat_substreams())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_substreams_with_parallelism_flattens_single_segment() {
  let values = Source::single(7_u32)
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .split_after(|_| true)
        .merge_substreams_with_parallelism(2)
        .expect("merge_substreams_with_parallelism"),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_substreams_with_parallelism_rejects_zero_parallelism() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| true);
  let result = flow.merge_substreams_with_parallelism(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "parallelism", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn map_error_transforms_upstream_failure() {
  let result = Source::<u32, _>::failed(StreamError::Failed)
    .via(Flow::new().map_error(|_| StreamError::WouldBlock))
    .collect_values();
  assert_eq!(result, Err(StreamError::WouldBlock));
}

#[test]
fn on_error_continue_resumes_after_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .via(Flow::new().on_error_continue())
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn on_error_resume_alias_resumes_after_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .via(Flow::new().on_error_resume())
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn on_error_continue_if_with_invokes_consumer_for_matching_failure() {
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let captured = observed.clone();
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .via(Flow::new().on_error_continue_if_with(
    |error| matches!(error, StreamError::Failed),
    move |error| {
      captured.lock().push(error.clone());
    },
  ))
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
  assert_eq!(observed.lock().as_slice(), &[StreamError::Failed]);
}

#[test]
fn on_error_complete_stops_after_matching_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .via(Flow::new().on_error_complete())
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn on_error_complete_if_stops_on_matching_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .via(Flow::new().on_error_complete_if(|error| matches!(error, StreamError::Failed)))
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn recover_replaces_upstream_failure_with_fallback() {
  let values = Source::<u32, _>::failed(StreamError::Failed)
    .via(Flow::new().recover(|_| Some(9_u32)))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![9_u32]);
}

#[test]
fn recover_drops_later_elements_after_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .via(Flow::new().recover(|_| Some(9_u32)))
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 9_u32]);
}

#[test]
fn recover_with_alias_switches_to_recovery_source() {
  let values = Source::<u32, _>::failed(StreamError::Failed)
    .via(Flow::new().recover_with(|_| Some(Source::from_array([8_u32, 9_u32]))))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![8_u32, 9_u32]);
}

#[test]
fn recover_with_retries_fails_when_retry_budget_is_exhausted() {
  let result = Source::<u32, _>::failed(StreamError::Failed)
    .via(Flow::new().recover_with_retries(0, |_| Some(Source::single(9_u32))))
    .collect_values();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn recover_with_retries_switches_recovery_sources_incrementally() {
  let mut attempts = 0_u8;
  let values = Source::<u32, _>::failed(StreamError::Failed)
    .via(Flow::new().recover_with_retries(2, move |_| {
      attempts = attempts.saturating_add(1);
      if attempts == 1 {
        Some(Source::<u32, _>::from_logic(
          StageKind::Custom,
          FailureSequenceSourceLogic::new(&[Ok(9_u32), Err(StreamError::Failed)]),
        ))
      } else {
        Some(Source::from_array([10_u32, 11_u32]))
      }
    }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![9_u32, 10_u32, 11_u32]);
}

#[test]
fn recover_with_retries_fails_after_consuming_retry_budget() {
  let result = Source::<u32, _>::failed(StreamError::Failed)
    .via(Flow::new().recover_with_retries(1, |_| {
      Some(Source::<u32, _>::from_logic(
        StageKind::Custom,
        FailureSequenceSourceLogic::new(&[Ok(9_u32), Err(StreamError::Failed)]),
      ))
    }))
    .collect_values();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn restart_flow_with_backoff_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().restart_flow_with_backoff(1, 3)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn on_failures_with_backoff_alias_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().on_failures_with_backoff(1, 3)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn with_backoff_alias_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().with_backoff(1, 3)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn with_backoff_and_context_alias_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().with_backoff_and_context(1, 3, "compat"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn restart_flow_with_settings_keeps_single_path_behavior() {
  let settings = RestartSettings::new(1, 4, 3)
    .with_random_factor_permille(250)
    .with_max_restarts_within_ticks(16)
    .with_jitter_seed(17);
  let values = Source::single(7_u32)
    .via(Flow::new().restart_flow_with_settings(settings))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn supervision_variants_keep_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().supervision_stop().supervision_resume().supervision_restart())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn zip_logic_on_restart_clears_pending_state() {
  let mut logic = super::ZipLogic::<u32> { fan_in: 2, edge_slots: Vec::new(), pending: Vec::new() };

  let first = logic.apply_with_edge(0, Box::new(1_u32)).expect("first apply");
  assert!(first.is_empty());

  logic.on_restart().expect("restart");

  let second = logic.apply_with_edge(1, Box::new(2_u32)).expect("second apply");
  assert!(second.is_empty());
}

#[test]
fn concat_logic_on_restart_clears_pending_state() {
  let mut logic = super::ConcatLogic::<u32> {
    fan_in:      2,
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    active_slot: 0,
    source_done: false,
  };

  let from_left = logic.apply_with_edge(0, Box::new(1_u32)).expect("left apply");
  assert_eq!(from_left.len(), 1);
  let initial = logic.apply_with_edge(1, Box::new(9_u32)).expect("right apply");
  assert!(initial.is_empty());
  logic.on_source_done().expect("source done");

  logic.on_restart().expect("restart");

  let drained = logic.drain_pending().expect("drain");
  assert!(drained.is_empty());
}

#[test]
fn concat_source_logic_on_restart_keeps_secondary_stream() {
  let mut logic = super::ConcatSourceLogic::<u32, StreamNotUsed> {
    secondary:         Some(Source::from_array([7_u32, 8_u32])),
    secondary_runtime: None,
    pending:           VecDeque::new(),
    source_done:       false,
  };

  logic.on_source_done().expect("source done");
  logic.on_restart().expect("restart");
  logic.on_source_done().expect("source done after restart");

  let mut values = Vec::new();
  loop {
    let outputs = logic.drain_pending().expect("drain");
    if outputs.is_empty() {
      break;
    }
    values.extend(outputs.into_iter().map(|output| *output.downcast::<u32>().expect("u32")));
  }
  assert_eq!(values, vec![7_u32, 8_u32]);
}

#[test]
fn secondary_source_bridge_emits_single_value() {
  let mut bridge = super::SecondarySourceBridge::new(Source::single(7_u32)).expect("bridge");

  assert_eq!(bridge.poll_next().expect("poll_next"), Some(7_u32));
}

#[test]
fn secondary_source_bridge_supports_multi_outlet_inner_source() {
  let mut bridge =
    super::SecondarySourceBridge::new(Source::single(7_u32).broadcast(2).expect("broadcast")).expect("bridge");

  assert_eq!(bridge.poll_next().expect("poll_next first"), Some(7_u32));
  assert_eq!(bridge.poll_next().expect("poll_next second"), Some(7_u32));
}

#[test]
fn prepend_source_logic_on_restart_keeps_secondary_stream() {
  let mut logic = super::PrependSourceLogic::<u32, StreamNotUsed> {
    secondary:         Some(Source::from_array([1_u32, 2_u32])),
    secondary_runtime: None,
    pending_secondary: VecDeque::new(),
    pending_primary:   VecDeque::new(),
  };

  let _ = logic.drain_pending().expect("drain before restart");
  logic.on_restart().expect("restart");

  let mut values = Vec::new();
  loop {
    let outputs = logic.drain_pending().expect("drain");
    if outputs.is_empty() {
      break;
    }
    values.extend(outputs.into_iter().map(|output| *output.downcast::<u32>().expect("u32")));
  }
  assert!(!values.is_empty(), "secondary stream should remain available after restart");
}

#[test]
fn or_else_source_logic_on_restart_keeps_secondary_stream() {
  let mut logic = super::OrElseSourceLogic::<u32, StreamNotUsed> {
    secondary:         Some(Source::from_array([9_u32, 10_u32])),
    secondary_runtime: None,
    pending_secondary: VecDeque::new(),
    emitted_primary:   false,
    source_done:       false,
  };

  logic.on_source_done().expect("source done");
  logic.on_restart().expect("restart");
  logic.on_source_done().expect("source done after restart");

  let mut values = Vec::new();
  loop {
    let outputs = logic.drain_pending().expect("drain");
    if outputs.is_empty() {
      break;
    }
    values.extend(outputs.into_iter().map(|output| *output.downcast::<u32>().expect("u32")));
  }
  assert_eq!(values, vec![9_u32, 10_u32]);
}

#[test]
fn zip_with_index_logic_on_restart_resets_counter() {
  let mut logic = super::ZipWithIndexLogic::<u32> { next_index: 0, _pd: core::marker::PhantomData };
  let first = logic.apply(Box::new(10_u32)).expect("first apply");
  let second = logic.apply(Box::new(11_u32)).expect("second apply");
  assert_eq!(first.len(), 1);
  assert_eq!(second.len(), 1);

  logic.on_restart().expect("restart");

  let after_restart = logic.apply(Box::new(12_u32)).expect("after restart apply");
  assert_eq!(after_restart.len(), 1);
}

#[test]
fn stateful_map_logic_on_restart_recreates_mapper() {
  let factory = || {
    let mut sum = 0_u32;
    move |value: u32| {
      sum = sum.saturating_add(value);
      sum
    }
  };
  let mapper = factory();
  let mut logic = super::StatefulMapLogic::<u32, u32, _, _> { factory, mapper, _pd: core::marker::PhantomData };

  let first = logic.apply(Box::new(1_u32)).expect("first apply");
  let second = logic.apply(Box::new(2_u32)).expect("second apply");
  assert_eq!(first.len(), 1);
  assert_eq!(second.len(), 1);

  logic.on_restart().expect("restart");

  let third = logic.apply(Box::new(3_u32)).expect("third apply");
  assert_eq!(third.len(), 1);

  let third_value = *third.into_iter().next().expect("third value").downcast::<u32>().expect("u32");
  assert_eq!(third_value, 3_u32);
}

#[test]
fn stateful_map_concat_logic_on_restart_recreates_mapper() {
  let factory = || {
    let mut sum = 0_u32;
    move |value: u32| {
      sum = sum.saturating_add(value);
      [sum]
    }
  };
  let mapper = factory();
  let mut logic =
    super::StatefulMapConcatLogic::<u32, u32, _, _, [u32; 1]> { factory, mapper, _pd: core::marker::PhantomData };

  let first = logic.apply(Box::new(1_u32)).expect("first apply");
  let second = logic.apply(Box::new(2_u32)).expect("second apply");
  assert_eq!(first.len(), 1);
  assert_eq!(second.len(), 1);

  logic.on_restart().expect("restart");

  let third = logic.apply(Box::new(3_u32)).expect("third apply");
  let third_value = *third.into_iter().next().expect("third value").downcast::<u32>().expect("u32");
  assert_eq!(third_value, 3_u32);
}

#[test]
fn collect_type_collects_convertible_values() {
  let values = Source::from_array([1_i32, -1_i32, 2_i32])
    .via(Flow::new().collect_type::<u32>())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn fold_async_emits_running_accumulation_when_future_is_ready() {
  let values = Source::from_array([1_u32, 2, 3])
    .via(Flow::new().fold_async(0_u32, |acc, value| async move { acc + value }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn ask_alias_maps_values_asynchronously() {
  let flow = Flow::new().ask(1, |value: u32| async move { value + 1 }).expect("ask");
  let values = Source::from_array([1_u32, 2_u32]).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![2_u32, 3_u32]);
}

#[test]
fn ask_with_status_alias_maps_values_asynchronously() {
  let flow = Flow::new().ask_with_status(1, |value: u32| async move { value + 2 }).expect("ask_with_status");
  let values = Source::from_array([1_u32, 2_u32]).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn ask_with_context_preserves_context_and_maps_value() {
  let flow = Flow::<(u32, u32), (u32, u32), StreamNotUsed>::new()
    .ask_with_context(1, |value| async move { value + 10 })
    .expect("ask_with_context");
  let values = Source::from_array([(7_u32, 1_u32), (8_u32, 2_u32)]).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![(7_u32, 11_u32), (8_u32, 12_u32)]);
}

#[test]
fn ask_with_status_and_context_preserves_context_and_maps_value() {
  let flow = Flow::<(u32, u32), (u32, u32), StreamNotUsed>::new()
    .ask_with_status_and_context(1, |value| async move { value + 20 })
    .expect("ask_with_status_and_context");
  let values = Source::from_array([(7_u32, 1_u32), (8_u32, 2_u32)]).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![(7_u32, 21_u32), (8_u32, 22_u32)]);
}

#[test]
fn watch_alias_keeps_single_path_behavior() {
  let values = Source::from_array([1_u32, 2_u32]).via(Flow::new().watch()).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn operator_catalog_lookup_returns_contract_for_supported_operator() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::GROUP_BY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::GROUP_BY);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3", "2.1", "2.2"]);
}

#[test]
fn operator_catalog_lookup_returns_filter_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FILTER).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FILTER);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_filter_not_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FILTER_NOT).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FILTER_NOT);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_empty_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::EMPTY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::EMPTY);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_from_option_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FROM_OPTION).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FROM_OPTION);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_from_array_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FROM_ARRAY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FROM_ARRAY);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_from_iterator_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FROM_ITERATOR).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FROM_ITERATOR);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_flatten_optional_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FLATTEN_OPTIONAL).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FLATTEN_OPTIONAL);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_stateful_map_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::STATEFUL_MAP).expect("lookup");
  assert_eq!(contract.key, OperatorKey::STATEFUL_MAP);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_map_async_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::MAP_ASYNC).expect("lookup");
  assert_eq!(contract.key, OperatorKey::MAP_ASYNC);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3", "7.1", "7.2", "7.3", "7.4"]);
}

#[test]
fn operator_catalog_lookup_returns_stateful_map_concat_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::STATEFUL_MAP_CONCAT).expect("lookup");
  assert_eq!(contract.key, OperatorKey::STATEFUL_MAP_CONCAT);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_rejects_unknown_operator() {
  let catalog = DefaultOperatorCatalog::new();
  let result = catalog.lookup(OperatorKey::new("unsupported_operator"));
  assert_eq!(result, Err(StreamDslError::UnsupportedOperator { key: OperatorKey::new("unsupported_operator") }));
}

#[test]
#[allow(deprecated)]
fn detach_preserves_elements_and_order() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().detach())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn log_passes_elements_through_unchanged_and_inserts_logging_stage() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().log("test"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);

  let (plan, completion) =
    Source::single(7_u32).via(Flow::new().log("log-stage")).into_mat(Sink::head(), KeepRight).into_parts();
  assert_eq!(completion.poll(), Completion::Pending);
  assert_eq!(plan.flow_order.len(), 1);
  assert!(matches!(
    plan.stages[plan.flow_order[0]],
    StageDefinition::Flow(ref definition) if definition.kind == StageKind::FlowLog
  ));
}

#[test]
fn log_does_not_bypass_upstream_source_supervision_resume() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Err(StreamError::Failed), Ok(7_u32)]),
  )
  .supervision_resume()
  .via(Flow::new().log("log-stage"))
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn log_with_marker_passes_elements_through_unchanged_and_inserts_logging_stage() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().log_with_marker("test", "marker"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);

  let (plan, completion) = Source::single(7_u32)
    .via(Flow::new().log_with_marker("log-stage", "marker"))
    .into_mat(Sink::head(), KeepRight)
    .into_parts();
  assert_eq!(completion.poll(), Completion::Pending);
  assert_eq!(plan.flow_order.len(), 1);
  assert!(matches!(
    plan.stages[plan.flow_order[0]],
    StageDefinition::Flow(ref definition) if definition.kind == StageKind::FlowLog
  ));
}

#[test]
fn log_and_log_with_marker_store_attributes() {
  let (graph, _mat) = Flow::<u32, u32, StreamNotUsed>::new().log("log-stage").into_parts();
  assert_eq!(graph.attributes().names(), &[alloc::string::String::from("log-stage")]);

  let (graph, _mat) = Flow::<u32, u32, StreamNotUsed>::new().log_with_marker("log-stage", "marker").into_parts();
  assert_eq!(graph.attributes().names(), &[
    alloc::string::String::from("log-stage"),
    alloc::string::String::from("marker")
  ]);
}

#[test]
#[cfg(feature = "compression")]
fn compression_operators_round_trip_bytes_and_store_attributes() {
  let payload = vec![1_u8, 2, 3, 3, 3, 4, 5];
  let values = Source::single(payload.clone())
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip().gzip_decompress())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![payload.clone()]);

  let values = Source::single(payload.clone())
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().deflate().inflate())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![payload]);

  let (graph, _mat) = Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip().inflate().into_parts();
  assert_eq!(graph.attributes().names(), &[
    alloc::string::String::from("compression:gzip"),
    alloc::string::String::from("compression:inflate"),
  ]);
}

#[cfg(feature = "compression")]
fn crc32_for_gzip_test(bytes: &[u8]) -> u32 {
  let mut crc = 0xffff_ffff_u32;
  for &byte in bytes {
    crc ^= u32::from(byte);
    for _ in 0..8 {
      let mask = (!((crc & 1).wrapping_sub(1))) & 0xedb8_8320;
      crc = (crc >> 1) ^ mask;
    }
  }
  !crc
}

#[cfg(feature = "compression")]
fn gzip_member_with_filename(payload: &[u8], filename: &str) -> Vec<u8> {
  let mut output = Vec::new();
  output.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03]);
  output.extend_from_slice(filename.as_bytes());
  output.push(0x00);
  output.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(payload, 6));
  output.extend_from_slice(&crc32_for_gzip_test(payload).to_le_bytes());
  output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
  output
}

#[test]
#[cfg(feature = "compression")]
fn gzip_decompress_accepts_member_with_filename_header() {
  let payload = b"gzip filename header payload".to_vec();
  let encoded = gzip_member_with_filename(&payload, "payload.bin");
  let values = Source::single(encoded)
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip_decompress())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![payload]);
}

#[test]
#[cfg(feature = "compression")]
fn gzip_decompress_accepts_standard_gzip_payload() {
  let encoded = vec![
    0x1f, 0x8b, 0x08, 0x00, 0xf6, 0x05, 0xaf, 0x69, 0x00, 0x03, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0xd7, 0x4d, 0xaf, 0xca,
    0x2c, 0xd0, 0xcd, 0xcc, 0x2b, 0x49, 0x2d, 0xca, 0x2f, 0x00, 0x00, 0xdf, 0x38, 0x73, 0x91, 0x12, 0x00, 0x00, 0x00,
  ];
  let values = Source::single(encoded)
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip_decompress())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![b"hello-gzip-interop".to_vec()]);
}

#[test]
#[cfg(feature = "compression")]
fn gzip_emits_raw_deflate_payload() {
  let payload = b"gzip-raw-deflate-check".to_vec();
  let encoded = Source::single(payload.clone())
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip())
    .collect_values()
    .expect("collect_values")
    .pop()
    .expect("encoded payload");
  let payload_end = encoded.len().saturating_sub(8);
  let deflate_payload = &encoded[10..payload_end];
  assert_eq!(miniz_oxide::inflate::decompress_to_vec(deflate_payload).expect("raw deflate payload"), payload);
  assert!(
    miniz_oxide::inflate::decompress_to_vec_zlib(deflate_payload).is_err(),
    "gzip payload must not be zlib-wrapped"
  );
}

#[test]
#[cfg(feature = "compression")]
fn inflate_rejects_payload_exceeding_decompression_limit() {
  let payload = vec![0x5a_u8; FLOW_DECOMPRESSION_MAX_BYTES_DEFAULT.saturating_add(1)];
  let encoded = miniz_oxide::deflate::compress_to_vec(&payload, 6);

  let result = Source::single(encoded).via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().inflate()).collect_values();

  assert!(matches!(result, Err(StreamError::CompressionError { .. })));
}

#[test]
#[cfg(feature = "compression")]
fn gzip_decompress_rejects_payload_exceeding_decompression_limit() {
  let payload = vec![0x33_u8; FLOW_DECOMPRESSION_MAX_BYTES_DEFAULT.saturating_add(1)];
  let encoded = Source::single(payload)
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip())
    .collect_values()
    .expect("collect_values")
    .pop()
    .expect("encoded payload");

  let result =
    Source::single(encoded).via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip_decompress()).collect_values();

  assert!(matches!(result, Err(StreamError::CompressionError { kind: "gzip_too_large" })));
}

#[test]
#[cfg(feature = "compression")]
fn gzip_decompress_rejects_payload_exceeding_decompression_limit_with_spoofed_isize() {
  let payload = vec![0x44_u8; FLOW_DECOMPRESSION_MAX_BYTES_DEFAULT.saturating_add(1)];
  let mut encoded = Source::single(payload)
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip())
    .collect_values()
    .expect("collect_values")
    .pop()
    .expect("encoded payload");
  let trailer_start = encoded.len().saturating_sub(4);
  encoded[trailer_start..].copy_from_slice(&0_u32.to_le_bytes());

  let result =
    Source::single(encoded).via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip_decompress()).collect_values();

  assert!(matches!(result, Err(StreamError::CompressionError { kind: "gzip_too_large" })));
}

#[test]
#[cfg(feature = "compression")]
fn inflate_accepts_payload_larger_than_compression_chunk_default() {
  let payload = vec![0x6a_u8; crate::core::dsl::Compression::MAX_BYTES_PER_CHUNK_DEFAULT.saturating_add(1)];
  let encoded = miniz_oxide::deflate::compress_to_vec(&payload, 6);

  let result = Source::single(encoded).via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().inflate()).collect_values();

  assert_eq!(result.expect("collect_values"), vec![payload]);
}

#[test]
#[cfg(feature = "compression")]
fn gzip_decompress_accepts_payload_larger_than_compression_chunk_default() {
  let payload = vec![0x7b_u8; crate::core::dsl::Compression::MAX_BYTES_PER_CHUNK_DEFAULT.saturating_add(1)];
  let encoded = Source::single(payload.clone())
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip())
    .collect_values()
    .expect("collect_values")
    .pop()
    .expect("encoded payload");

  let result =
    Source::single(encoded).via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip_decompress()).collect_values();

  assert_eq!(result.expect("collect_values"), vec![payload]);
}

#[test]
fn limit_weighted_stops_before_exceeding_budget() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[2, 2, 1]))
    .via(Flow::new().limit_weighted(3, |value| *value as usize))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32]);
}

#[test]
fn limit_weighted_requests_shutdown_after_exceeding_budget() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let values =
    Source::<u32, _>::from_logic(StageKind::Custom, CountingSequenceSourceLogic::new(&[2, 2, 1], pulls.clone()))
      .via(Flow::new().limit_weighted(3, |value| *value as usize))
      .collect_values()
      .expect("collect_values");
  assert_eq!(values, vec![2_u32]);
  assert_eq!(*pulls.lock(), 2_usize);
}

#[test]
fn grouped_weighted_within_uses_weight_budget() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[2, 1, 2]))
    .via(Flow::new().grouped_weighted_within(3, 10, |value| *value as usize).expect("grouped_weighted_within"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![2_u32, 1_u32], vec![2_u32]]);
}

#[test]
fn grouped_weighted_within_flushes_on_weight_add_overflow() {
  let values = Source::from_array([usize::MAX - 1, 2_usize])
    .via(Flow::new().grouped_weighted_within(usize::MAX, 10, |value| *value).expect("grouped_weighted_within"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![usize::MAX - 1], vec![2_usize]]);
}

#[test]
fn batch_weighted_uses_weight_budget() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[2, 1, 2]))
    .via(Flow::new().batch_weighted(3, |value| *value as usize).expect("batch_weighted"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![2_u32, 1_u32], vec![2_u32]]);
}

#[test]
fn map_async_partitioned_rejects_zero_parallelism() {
  let result = Flow::<u32, u32, StreamNotUsed>::new().map_async_partitioned(
    0,
    |value: &u32| (*value as usize) % 2,
    |value, _partition| async move { value },
  );
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "parallelism", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn flat_map_prefix_uses_prefix_to_build_tail_flow() {
  let values = Source::from_array([1_u32, 2, 3, 4])
    .via(Flow::new().flat_map_prefix(2, |prefix| {
      let prefix_sum = prefix.into_iter().sum::<u32>();
      Flow::new().map(move |value: u32| value.saturating_add(prefix_sum))
    }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![6_u32, 7_u32]);
}

#[test]
fn flat_map_prefix_accepts_zero_prefix() {
  let values = Source::from_array([1_u32, 2])
    .via(Flow::new().flat_map_prefix(0, |_prefix| Flow::new().map(|value: u32| value.saturating_add(5))))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![6_u32, 7_u32]);
}

#[test]
fn prefix_and_tail_emits_prefix_and_remaining_tail() {
  let values =
    Source::from_array([1_u32, 2, 3, 4]).via(Flow::new().prefix_and_tail(2)).collect_values().expect("collect_values");
  assert_eq!(values.len(), 1);
  let (prefix, tail): (Vec<u32>, TailSource<u32>) = values.into_iter().next().expect("prefix and tail");
  assert_eq!(prefix, vec![1_u32, 2_u32]);
  assert_eq!(tail.collect_values().expect("tail values"), vec![3_u32, 4_u32]);
}

#[test]
fn prefix_and_tail_accepts_zero_prefix() {
  let values =
    Source::from_array([7_u32, 8]).via(Flow::new().prefix_and_tail(0)).collect_values().expect("collect_values");
  assert_eq!(values.len(), 1);
  let (prefix, tail): (Vec<u32>, TailSource<u32>) = values.into_iter().next().expect("prefix and tail");
  assert_eq!(prefix, vec![]);
  assert_eq!(tail.collect_values().expect("tail values"), vec![7_u32, 8_u32]);
}

#[test]
fn do_on_first_invokes_callback_on_first_element_only() {
  use core::sync::atomic::{AtomicU32, Ordering};

  use fraktor_utils_rs::core::sync::ArcShared;

  let counter = ArcShared::new(AtomicU32::new(0));
  let counter_clone = counter.clone();
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[10, 20, 30]))
    .via(Flow::new().do_on_first(move |_value| {
      counter_clone.fetch_add(1, Ordering::Relaxed);
    }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32, 20_u32, 30_u32]);
  assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[test]
fn conflate_preserves_elements_when_upstream_is_not_bursty() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().conflate(|acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn conflate_with_seed_applies_seed_and_aggregate() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().conflate_with_seed(|value| value + 10, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![11_u32, 12_u32, 13_u32]);
}

#[test]
fn conflate_aggregates_bursty_upstream_values() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .via(Flow::new().map_concat(|value: u32| vec![value, value.saturating_mul(10)]))
    .via(Flow::new().conflate(|acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![11_u32, 22_u32]);
}

#[test]
fn conflate_with_seed_aggregates_bursty_upstream_values() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .via(Flow::new().map_concat(|value: u32| vec![value, value.saturating_mul(10)]))
    .via(Flow::new().conflate_with_seed(|value| value + 100, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![111_u32, 122_u32]);
}

#[test]
fn conflate_accepts_non_clone_output_type() {
  #[derive(Debug, PartialEq, Eq)]
  struct NonClone(u32);

  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().map(NonClone))
    .via(Flow::new().conflate(|acc: NonClone, value: NonClone| NonClone(acc.0 + value.0)))
    .collect_values()
    .expect("collect_values");

  assert_eq!(values, vec![NonClone(1), NonClone(2), NonClone(3)]);
}

#[test]
fn expand_and_extrapolate_share_expand_behavior() {
  let expand_values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .via(Flow::new().expand(|value: &u32| vec![*value, value.saturating_mul(10)]))
    .collect_values()
    .expect("collect_values");
  let extrapolate_values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .via(Flow::new().extrapolate(|value: &u32| vec![*value, value.saturating_mul(10)]))
    .collect_values()
    .expect("collect_values");
  assert_eq!(expand_values, vec![1_u32, 2_u32]);
  assert_eq!(expand_values, extrapolate_values);
}

#[test]
fn expand_and_extrapolate_emit_extrapolated_values_during_idle_ticks() {
  let mut logic = super::ExpandLogic::<u32, _> {
    expander:                |value: &u32| vec![*value, value.saturating_mul(10)],
    last:                    None,
    pending:                 None,
    tick_count:              0,
    last_input_tick:         None,
    last_extrapolation_tick: None,
    source_done:             false,
  };

  logic.on_tick(1).expect("tick 1");
  let first = logic.apply(Box::new(1_u32)).expect("apply first");
  let first_values: Vec<u32> = first.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(first_values, vec![1_u32]);

  let remaining_same_tick = logic.drain_pending().expect("drain tick 1");
  let remaining_same_tick_values: Vec<u32> =
    remaining_same_tick.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(remaining_same_tick_values, vec![10_u32]);

  logic.on_tick(2).expect("tick 2");
  let extrapolated = logic.drain_pending().expect("drain tick 2");
  let extrapolated_values: Vec<u32> =
    extrapolated.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(extrapolated_values, vec![1_u32]);

  let extrapolated_remaining = logic.drain_pending().expect("drain tick 2 remaining");
  let extrapolated_remaining_values: Vec<u32> =
    extrapolated_remaining.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(extrapolated_remaining_values, vec![10_u32]);

  logic.on_tick(3).expect("tick 3");
  let extrapolated_again = logic.drain_pending().expect("drain tick 3");
  let extrapolated_again_values: Vec<u32> =
    extrapolated_again.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(extrapolated_again_values, vec![1_u32]);

  let extrapolated_again_remaining = logic.drain_pending().expect("drain tick 3 remaining");
  let extrapolated_again_remaining_values: Vec<u32> =
    extrapolated_again_remaining.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(extrapolated_again_remaining_values, vec![10_u32]);

  logic.on_source_done().expect("source done");
  logic.on_tick(4).expect("tick 4");
  assert!(logic.drain_pending().expect("drain tick 4").is_empty());
}

#[test]
fn expand_and_extrapolate_do_not_hang_with_infinite_iterators() {
  let mut logic = super::ExpandLogic::<u32, _> {
    expander:                |value: &u32| core::iter::repeat(*value),
    last:                    None,
    pending:                 None,
    tick_count:              0,
    last_input_tick:         None,
    last_extrapolation_tick: None,
    source_done:             false,
  };

  logic.on_tick(1).expect("tick 1");
  let first = logic.apply(Box::new(1_u32)).expect("apply first");
  let first_values: Vec<u32> = first.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(first_values, vec![1_u32]);
  assert!(logic.can_accept_input());

  let second = logic.drain_pending().expect("drain second");
  let second_values: Vec<u32> = second.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(second_values, vec![1_u32]);
  assert!(logic.can_accept_input());

  logic.on_tick(2).expect("tick 2");
  let extrapolated = logic.drain_pending().expect("drain extrapolated");
  let extrapolated_values: Vec<u32> =
    extrapolated.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(extrapolated_values, vec![1_u32]);

  let replaced = logic.apply(Box::new(2_u32)).expect("apply replacement");
  let replaced_values: Vec<u32> = replaced.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(replaced_values, vec![2_u32]);

  let replaced_following = logic.drain_pending().expect("drain replacement");
  let replaced_following_values: Vec<u32> =
    replaced_following.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(replaced_following_values, vec![2_u32]);

  logic.on_source_done().expect("source done");
  logic.on_tick(3).expect("tick 3");
  assert!(logic.drain_pending().expect("drain after source done").is_empty());
  assert!(!logic.has_pending_output());
}

#[test]
fn grouped_within_preserves_size_grouping_behavior() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]))
    .via(Flow::new().grouped_within(2, 10).expect("grouped_within"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32], vec![5_u32]]);
}

#[test]
fn grouped_within_flushes_when_tick_window_expires() {
  let schedule = [Some(1_u32), None, None, Some(2_u32), Some(3_u32)];
  let values = Source::<u32, _>::from_logic(StageKind::Custom, PulsedSourceLogic::new(&schedule))
    .via(Flow::new().grouped_within(10, 2).expect("grouped_within"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32], vec![2_u32, 3_u32]]);
}

#[test]
fn grouped_within_expires_window_at_tick_boundary() {
  let schedule = [Some(1_u32), Some(2_u32)];
  let values = Source::<u32, _>::from_logic(StageKind::Custom, PulsedSourceLogic::new(&schedule))
    .via(Flow::new().grouped_within(10, 1).expect("grouped_within"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32], vec![2_u32]]);
}

#[test]
fn grouped_within_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.grouped_within(2, 0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn fold_emits_running_accumulation_without_initial() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().fold(0_u32, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn reduce_folds_with_first_element_as_seed() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().reduce(|acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn reduce_single_element_emits_that_element() {
  let values =
    Source::single(42_u32).via(Flow::new().reduce(|acc, value| acc + value)).collect_values().expect("collect_values");
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn fold_on_empty_stream_emits_nothing() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().fold(0_u32, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert!(values.is_empty());
}

#[test]
fn reduce_on_empty_stream_emits_nothing() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().reduce(|acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert!(values.is_empty());
}

#[test]
fn do_on_first_does_not_fire_on_empty_stream() {
  use core::sync::atomic::{AtomicU32, Ordering};

  use fraktor_utils_rs::core::sync::ArcShared;

  let counter = ArcShared::new(AtomicU32::new(0));
  let counter_clone = counter.clone();
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().do_on_first(move |_value| {
      counter_clone.fetch_add(1, Ordering::Relaxed);
    }))
    .collect_values()
    .expect("collect_values");
  assert!(values.is_empty());
  assert_eq!(counter.load(Ordering::Relaxed), 0);
}

#[test]
fn from_function_transforms_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::from_function(|x: u32| x + 1))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 3_u32, 4_u32]);
}

#[test]
fn named_passes_elements_through_unchanged() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().named("test-stage"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn also_to_mat_combines_materialized_values() {
  let (graph, (left_mat, right_mat)) =
    Flow::<u32, u32, StreamNotUsed>::new().also_to_mat(Sink::head(), KeepBoth).into_parts();
  let _ = graph;
  assert_eq!(left_mat, StreamNotUsed::new());
  assert_eq!(right_mat.poll(), Completion::Pending);
}

#[test]
fn also_to_mat_keeps_data_path_behavior() {
  let values = Source::single(1_u32)
    .via(Flow::new().map(|value: u32| value + 1).also_to_mat(Sink::ignore(), KeepBoth))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32]);
}

#[test]
fn also_to_mat_routes_elements_to_side_sink() {
  let (mut graph, side_completion) =
    Source::single(9_u32).via_mat(Flow::new().also_to_mat(Sink::head(), KeepRight), KeepRight).into_parts();
  let (sink_graph, downstream_completion) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");
  let mut idle_budget = 1024_usize;
  while !stream.state().is_terminal() {
    match stream.drive() {
      | DriveOutcome::Progressed => idle_budget = 1024,
      | DriveOutcome::Idle => {
        assert!(idle_budget > 0, "stream stalled");
        idle_budget = idle_budget.saturating_sub(1);
      },
    }
  }
  assert_eq!(side_completion.poll(), Completion::Ready(Ok(9_u32)));
  assert_eq!(downstream_completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn wire_tap_mat_combines_materialized_values_and_keeps_data_path_behavior() {
  let (graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new().wire_tap_mat(Sink::head(), KeepRight).into_parts();
  let _ = graph;
  assert_eq!(materialized.poll(), Completion::Pending);

  let values = Source::single(4_u32)
    .via(Flow::new().map(|value: u32| value + 1).wire_tap_mat(Sink::ignore(), KeepRight))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn wire_tap_mat_routes_elements_to_side_sink() {
  let (mut graph, side_completion) =
    Source::single(4_u32).via_mat(Flow::new().wire_tap_mat(Sink::head(), KeepRight), KeepRight).into_parts();
  let (sink_graph, downstream_completion) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");
  let mut idle_budget = 1024_usize;
  while !stream.state().is_terminal() {
    match stream.drive() {
      | DriveOutcome::Progressed => idle_budget = 1024,
      | DriveOutcome::Idle => {
        assert!(idle_budget > 0, "stream stalled");
        idle_budget = idle_budget.saturating_sub(1);
      },
    }
  }
  assert_eq!(side_completion.poll(), Completion::Ready(Ok(4_u32)));
  assert_eq!(downstream_completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn wire_tap_mat_preserves_all_main_path_elements_with_multiple_inputs() {
  // 準備: 複数要素を wire_tap_mat に流す（main path が全要素を受け取ることを検証）
  let values = Source::from_array([1_u32, 2, 3, 4, 5])
    .via(Flow::new().wire_tap_mat(Sink::ignore(), KeepLeft))
    .collect_values()
    .expect("collect_values");

  // 検証: main path は全要素を受け取る
  assert_eq!(values, vec![1_u32, 2, 3, 4, 5]);
}

#[test]
fn wire_tap_mat_callback_version_observes_all_elements() {
  // 準備: callback 版 wire_tap で全要素を観測する
  let observed = ArcShared::new(SpinSyncMutex::new(alloc::vec::Vec::<u32>::new()));
  let observed_clone = observed.clone();

  let values = Source::from_array([10_u32, 20, 30])
    .via(Flow::new().wire_tap(move |value| {
      observed_clone.lock().push(*value);
    }))
    .collect_values()
    .expect("collect_values");

  // 検証: main path は全要素を受け取り、callback も全要素を観測する
  assert_eq!(values, vec![10_u32, 20, 30]);
  assert_eq!(*observed.lock(), vec![10_u32, 20, 30]);
}

#[test]
fn wire_tap_mat_side_sink_receives_elements() {
  // 準備: wire_tap_mat で fold sink を使い、要素合計を検証
  let (mut graph, side_mat) = Source::from_array([1_u32, 2, 3])
    .via_mat(Flow::new().wire_tap_mat(Sink::fold(0_u32, |acc, value| acc + value), KeepRight), KeepRight)
    .into_parts();

  let (sink_graph, _downstream) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");
  let mut idle_budget = 1024_usize;
  while !stream.state().is_terminal() {
    match stream.drive() {
      | DriveOutcome::Progressed => idle_budget = 1024,
      | DriveOutcome::Idle => {
        assert!(idle_budget > 0, "stream stalled");
        idle_budget = idle_budget.saturating_sub(1);
      },
    }
  }

  // 検証: side sink が全要素の合計を受け取る
  assert_eq!(side_mat.poll(), Completion::Ready(Ok(6_u32)));
}

#[test]
fn monitor_mat_combines_materialized_values_and_keeps_data_path_behavior() {
  let (graph, _unused) = Flow::<u32, u32, StreamNotUsed>::new().into_parts();
  let flow: Flow<u32, u32, u32> = Flow::from_graph(graph, 21_u32);
  let (_graph, (left_mat, right_mat)) = flow.monitor_mat(KeepBoth).into_parts();
  assert_eq!(left_mat, 21_u32);
  assert_eq!(right_mat, FlowMonitorImpl::<u32>::new());

  let values = Source::single(4_u32)
    .via(Flow::new().map(|value: u32| value + 1).monitor_mat(KeepLeft))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn from_sink_and_source_mat_combines_materialized_values() {
  let sink = Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| 7_u32);
  let source = Source::single(99_u32).map_materialized_value(|_| 11_u32);

  let (_graph, (left_mat, right_mat)) =
    Flow::<u32, u32, StreamNotUsed>::from_sink_and_source_mat(sink, source, KeepBoth).into_parts();

  assert_eq!(left_mat, 7_u32);
  assert_eq!(right_mat, 11_u32);
}

#[test]
fn from_sink_and_source_mat_preserves_single_path_behavior() {
  let sink = Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| 1_u32);
  let source = Source::single(99_u32).map_materialized_value(|_| 2_u32);

  let values = Source::single(5_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::from_sink_and_source_mat(sink, source, KeepLeft))
    .collect_values()
    .expect("collect_values");

  // from_sink_and_source_mat emits elements from the embedded source, not from upstream
  assert_eq!(values, vec![99_u32]);
}

#[test]
fn from_sink_and_source_coupled_mat_keeps_requested_materialized_value() {
  let sink = Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| 13_u32);
  let source = Source::single(99_u32).map_materialized_value(|_| 17_u32);

  let (_graph, materialized) =
    Flow::<u32, u32, StreamNotUsed>::from_sink_and_source_coupled_mat(sink, source, KeepRight).into_parts();

  assert_eq!(materialized, 17_u32);
}

#[test]
fn from_sink_and_source_coupled_mat_accepts_non_send_materialized_values() {
  use alloc::rc::Rc;

  let sink = Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| Rc::new(13_u32));
  let source = Source::single(99_u32).map_materialized_value(|_| Rc::new(17_u32));

  let (_graph, (left_mat, right_mat)) =
    Flow::<u32, u32, StreamNotUsed>::from_sink_and_source_coupled_mat(sink, source, KeepBoth).into_parts();

  assert_eq!(*left_mat, 13_u32);
  assert_eq!(*right_mat, 17_u32);
}

#[test]
fn from_sink_and_source_coupled_mat_preserves_single_path_behavior() {
  let sink = Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| 3_u32);
  let source = Source::single(99_u32).map_materialized_value(|_| 4_u32);

  let values = Source::single(6_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::from_sink_and_source_coupled_mat(sink, source, KeepLeft))
    .collect_values()
    .expect("collect_values");

  // from_sink_and_source_coupled_mat emits elements from the embedded source, not from upstream
  assert_eq!(values, vec![99_u32]);
}

#[test]
fn flow_lazy_flow_passes_elements_through_factory_flow() {
  let values = Source::single(5_u32)
    .via(Flow::lazy_flow(|| Flow::new().map(|v: u32| v * 2)))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32]);
}

#[test]
fn flow_lazy_flow_defers_factory_call() {
  let values = Source::from_array([1_u32, 2, 3])
    .via(Flow::lazy_flow(|| Flow::new().map(|v: u32| v + 100)))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![101_u32, 102, 103]);
}

#[test]
fn flow_lazy_flow_with_identity_flow_passes_through() {
  let values = Source::from_array([1_u32, 2, 3])
    .via(Flow::lazy_flow(Flow::<u32, u32, StreamNotUsed>::new))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn flow_lazy_flow_with_chained_operations() {
  let values = Source::from_array([1_u32, 2, 3, 4, 5])
    .via(Flow::lazy_flow(|| Flow::new().map(|v: u32| v * 2).filter(|v: &u32| *v > 4)))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![6_u32, 8, 10]);
}

#[test]
fn flow_lazy_flow_with_empty_source() {
  let values = Source::<u32, _>::empty()
    .via(Flow::lazy_flow(|| Flow::new().map(|v: u32| v + 1)))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn flow_lazy_completion_stage_flow_delegates_to_lazy_flow() {
  let values = Source::single(7_u32)
    .via(Flow::lazy_completion_stage_flow(|| Flow::new().map(|v: u32| v + 3)))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32]);
}

#[test]
fn flow_lazy_future_flow_delegates_to_lazy_flow() {
  let values = Source::single(7_u32)
    .via(Flow::lazy_future_flow(|| Flow::new().map(|v: u32| v + 3)))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32]);
}

#[test]
fn flow_map_materialized_value_transforms_materialized_value_and_keeps_data_path_behavior() {
  let (graph, _unused) = Flow::<u32, u32, StreamNotUsed>::new().map(|value: u32| value + 1).into_parts();
  let flow: Flow<u32, u32, u32> = Flow::from_graph(graph, 10_u32);
  let (_graph, materialized) = flow.map_materialized_value(|value| value + 5).into_parts();
  assert_eq!(materialized, 15_u32);

  let values = Source::single(4_u32)
    .via(Flow::new().map(|value: u32| value + 1).map_materialized_value(|_| 1_u32))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

// --- backpressure_timeout ---

#[test]
fn backpressure_timeout_passes_elements_within_threshold() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().backpressure_timeout(100).expect("backpressure_timeout"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn backpressure_timeout_fails_when_backpressure_exceeds_threshold() {
  let result =
    Source::<u32, _>::from_logic(StageKind::Custom, PulsedSourceLogic::new(&[Some(1), None, None, None, None]))
      .via(Flow::new().backpressure_timeout(2).expect("backpressure_timeout"))
      .collect_values();
  assert!(matches!(result, Err(StreamError::Timeout { kind: "backpressure", ticks: 2 })));
}

#[test]
fn backpressure_timeout_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.backpressure_timeout(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- completion_timeout ---

#[test]
fn completion_timeout_passes_elements_within_threshold() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().completion_timeout(100).expect("completion_timeout"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn completion_timeout_fails_when_stream_exceeds_threshold() {
  let result = Source::<u32, _>::from_logic(StageKind::Custom, PulsedSourceLogic::new(&[None, None, None, Some(1)]))
    .via(Flow::new().completion_timeout(2).expect("completion_timeout"))
    .collect_values();
  assert!(matches!(result, Err(StreamError::Timeout { kind: "completion", ticks: 2 })));
}

#[test]
fn completion_timeout_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.completion_timeout(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- idle_timeout ---

#[test]
fn idle_timeout_passes_elements_within_threshold() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().idle_timeout(100).expect("idle_timeout"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn idle_timeout_fails_when_no_elements_within_threshold() {
  let result = Source::<u32, _>::from_logic(StageKind::Custom, PulsedSourceLogic::new(&[None, None, None, Some(1)]))
    .via(Flow::new().idle_timeout(2).expect("idle_timeout"))
    .collect_values();
  assert!(matches!(result, Err(StreamError::Timeout { kind: "idle", ticks: 2 })));
}

#[test]
fn idle_timeout_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.idle_timeout(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- initial_timeout ---

#[test]
fn initial_timeout_passes_elements_within_threshold() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().initial_timeout(100).expect("initial_timeout"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn initial_timeout_fails_when_first_element_exceeds_threshold() {
  let result = Source::<u32, _>::from_logic(StageKind::Custom, PulsedSourceLogic::new(&[None, None, None, Some(1)]))
    .via(Flow::new().initial_timeout(2).expect("initial_timeout"))
    .collect_values();
  assert!(matches!(result, Err(StreamError::Timeout { kind: "initial", ticks: 2 })));
}

#[test]
fn initial_timeout_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.initial_timeout(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- MergePreferredLogic テスト ---

#[test]
fn merge_preferred_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().merge_preferred(1).expect("merge_preferred"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_preferred_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.merge_preferred(0).is_err());
}

#[test]
fn merge_preferred_logic_prefers_slot_zero() {
  let mut logic = super::MergePreferredLogic::<u32> {
    fan_in:      2,
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    source_done: false,
  };

  // edge 1 が最初に接続 → partition_point により slot 0 に配置される
  let result = logic.apply_with_edge(1, Box::new(100_u32)).expect("edge 1");
  assert_eq!(result.len(), 1); // slot 0 から即座に取得

  // edge 1 に再度データ投入 → 即座に消費される
  let _ = logic.apply_with_edge(1, Box::new(200_u32)).expect("edge 1 second");
  // edge 0 が接続 → partition_point により slot 0 に挿入、edge 1 は slot 1 にシフト
  let result = logic.apply_with_edge(0, Box::new(10_u32)).expect("edge 0");
  // edge 0 のデータが slot 0（preferred）にあるため出力される
  assert_eq!(result.len(), 1);
  let value = result[0].downcast_ref::<u32>().expect("downcast");
  assert_eq!(*value, 10_u32);
}

#[test]
fn merge_preferred_logic_always_prefers_slot_zero_in_sequence() {
  let mut logic = super::MergePreferredLogic::<u32> {
    fan_in:      2,
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    source_done: false,
  };

  // 各apply_with_edgeで投入されたデータは即座に消費される（同時データなし）
  // このテストはシーケンシャルな投入・消費の基本動作を確認する
  let r1 = logic.apply_with_edge(0, Box::new(10_u32)).expect("edge 0 first");
  assert_eq!(r1.len(), 1);
  assert_eq!(*r1[0].downcast_ref::<u32>().expect("downcast"), 10_u32);

  let r2 = logic.apply_with_edge(1, Box::new(99_u32)).expect("edge 1");
  assert_eq!(r2.len(), 1);
  assert_eq!(*r2[0].downcast_ref::<u32>().expect("downcast"), 99_u32);

  let r3 = logic.apply_with_edge(0, Box::new(20_u32)).expect("edge 0 second");
  assert_eq!(r3.len(), 1);
  assert_eq!(*r3[0].downcast_ref::<u32>().expect("downcast"), 20_u32);
}

#[test]
fn merge_preferred_logic_prefers_slot_zero_with_simultaneous_data() {
  // 両スロットに同時にデータが存在する状態で、slot 0（preferred）が優先されることを検証
  let mut logic = super::MergePreferredLogic::<u32> {
    fan_in:      2,
    edge_slots:  vec![0, 1],
    pending:     vec![VecDeque::from([10, 20, 30]), VecDeque::from([100, 200, 300])],
    source_done: false,
  };

  // pop_preferred は常に slot 0 を優先する。
  // slot 0 のデータが全て消費されてから slot 1 のデータを取得する。
  let v1 = logic.pop_preferred().expect("pop 1");
  assert_eq!(v1, 10_u32);
  let v2 = logic.pop_preferred().expect("pop 2");
  assert_eq!(v2, 20_u32);
  let v3 = logic.pop_preferred().expect("pop 3");
  assert_eq!(v3, 30_u32);

  // slot 0 が空になったので slot 1 から取得
  let v4 = logic.pop_preferred().expect("pop 4");
  assert_eq!(v4, 100_u32);
  let v5 = logic.pop_preferred().expect("pop 5");
  assert_eq!(v5, 200_u32);
  let v6 = logic.pop_preferred().expect("pop 6");
  assert_eq!(v6, 300_u32);

  // 全消費後は None
  assert!(logic.pop_preferred().is_none());
}

#[test]
fn merge_preferred_logic_falls_back_to_secondary() {
  // slot 0（preferred）が空の場合、slot 1（secondary）から取得されることを検証
  let mut logic = super::MergePreferredLogic::<u32> {
    fan_in:      2,
    edge_slots:  vec![0, 1],
    pending:     vec![VecDeque::new(), VecDeque::from([100, 200])],
    source_done: false,
  };

  // slot 0 は空なので slot 1 にフォールバック
  let v1 = logic.pop_preferred().expect("pop 1");
  assert_eq!(v1, 100_u32);
  let v2 = logic.pop_preferred().expect("pop 2");
  assert_eq!(v2, 200_u32);

  assert!(logic.pop_preferred().is_none());
}

#[test]
fn merge_preferred_logic_on_restart_clears_state() {
  let mut logic = super::MergePreferredLogic::<u32> {
    fan_in:      2,
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    source_done: false,
  };

  let _ = logic.apply_with_edge(0, Box::new(1_u32)).expect("apply");
  logic.on_source_done().expect("source done");

  logic.on_restart().expect("restart");

  let drained = logic.drain_pending().expect("drain");
  assert!(drained.is_empty());
}

// --- MergePrioritizedLogic テスト ---

#[test]
fn merge_prioritized_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().merge_prioritized(1).expect("merge_prioritized"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_prioritized_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.merge_prioritized(0).is_err());
}

#[test]
fn merge_prioritized_n_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().merge_prioritized_n(1, &[1]).expect("merge_prioritized_n"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_prioritized_n_rejects_zero_priority() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.merge_prioritized_n(2, &[3, 0]).is_err());
}

#[test]
fn merge_prioritized_n_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.merge_prioritized_n(0, &[]).is_err());
}

#[test]
fn merge_prioritized_n_rejects_length_mismatch() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.merge_prioritized_n(3, &[3, 1]).is_err());
}

#[test]
fn merge_prioritized_logic_outputs_on_each_apply() {
  // シーケンシャルな投入・消費の基本動作を確認
  let mut logic = super::MergePrioritizedLogic::<u32> {
    fan_in:      2,
    priorities:  vec![3, 1],
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    credits:     Vec::new(),
    current:     0,
    source_done: false,
  };

  // 各apply_with_edgeで投入されたデータは即座に消費される
  let r1 = logic.apply_with_edge(0, Box::new(10_u32)).expect("edge 0 first");
  assert_eq!(r1.len(), 1);
  assert_eq!(*r1[0].downcast_ref::<u32>().expect("downcast"), 10_u32);

  let r2 = logic.apply_with_edge(1, Box::new(99_u32)).expect("edge 1 first");
  assert_eq!(r2.len(), 1);
  assert_eq!(*r2[0].downcast_ref::<u32>().expect("downcast"), 99_u32);

  let r3 = logic.apply_with_edge(0, Box::new(20_u32)).expect("edge 0 second");
  assert_eq!(r3.len(), 1);
  assert_eq!(*r3[0].downcast_ref::<u32>().expect("downcast"), 20_u32);

  // source_done後のdrain: 全要素は即座に出力されたため、drainは空
  logic.on_source_done().expect("source done");
  let drained = logic.drain_pending().expect("drain");
  assert!(drained.is_empty());
}

#[test]
fn merge_prioritized_logic_respects_weight_ratio() {
  // 重み [3, 1] で両スロットにデータが同時に存在する場合、
  // slot 0 から3つ → slot 1 から1つ のサイクルで取得されることを検証
  let mut logic = super::MergePrioritizedLogic::<u32> {
    fan_in:      2,
    priorities:  vec![3, 1],
    edge_slots:  vec![0, 1],
    pending:     vec![VecDeque::from([10, 20, 30, 40, 50, 60]), VecDeque::from([100, 200, 300, 400, 500, 600])],
    credits:     vec![3, 1],
    current:     0,
    source_done: false,
  };

  let mut results = Vec::new();
  for _ in 0..8 {
    if let Some(v) = logic.pop_prioritized() {
      results.push(v);
    }
  }

  // クレジットベースラウンドロビン:
  // サイクル1: slot 0 × 3 (credit=3) → slot 1 × 1 (credit=1)
  // サイクル2: refill → slot 0 × 3 → slot 1 × 1
  assert_eq!(results, vec![10, 20, 30, 100, 40, 50, 60, 200]);
}

#[test]
fn merge_prioritized_logic_equal_weights_alternates() {
  // 等重み [1, 1] で両スロットにデータがある場合、交互に取得されることを検証
  let mut logic = super::MergePrioritizedLogic::<u32> {
    fan_in:      2,
    priorities:  vec![1, 1],
    edge_slots:  vec![0, 1],
    pending:     vec![VecDeque::from([10, 20, 30]), VecDeque::from([100, 200, 300])],
    credits:     vec![1, 1],
    current:     0,
    source_done: false,
  };

  let mut results = Vec::new();
  for _ in 0..6 {
    if let Some(v) = logic.pop_prioritized() {
      results.push(v);
    }
  }

  // 等重み: slot 0 × 1 → slot 1 × 1 → refill → 繰り返し
  assert_eq!(results, vec![10, 100, 20, 200, 30, 300]);
}

#[test]
fn merge_prioritized_logic_on_restart_clears_state() {
  let mut logic = super::MergePrioritizedLogic::<u32> {
    fan_in:      2,
    priorities:  vec![1, 1],
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    credits:     Vec::new(),
    current:     0,
    source_done: false,
  };

  let _ = logic.apply_with_edge(0, Box::new(1_u32)).expect("apply");
  logic.on_source_done().expect("source done");

  logic.on_restart().expect("restart");

  let drained = logic.drain_pending().expect("drain");
  assert!(drained.is_empty());
}

// --- MergeSortedLogic テスト ---

#[test]
fn merge_sorted_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().merge_sorted(1).expect("merge_sorted"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_sorted_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.merge_sorted(0).is_err());
}

#[test]
fn merge_sorted_logic_emits_minimum_value() {
  let mut logic = super::MergeSortedLogic::<u32> {
    fan_in:      2,
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    source_done: false,
  };

  // edge 0 に大きい値
  let result = logic.apply_with_edge(0, Box::new(10_u32)).expect("edge 0");
  assert!(result.is_empty()); // もう1つのスロットが空なので待機

  // edge 1 に小さい値 → 全スロットに要素があるので最小値を出力
  let result = logic.apply_with_edge(1, Box::new(3_u32)).expect("edge 1");
  assert_eq!(result.len(), 1);
  let value = result[0].downcast_ref::<u32>().expect("downcast");
  assert_eq!(*value, 3_u32);
}

#[test]
fn merge_sorted_logic_drain_emits_sorted_order() {
  let mut logic = super::MergeSortedLogic::<u32> {
    fan_in:      2,
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    source_done: false,
  };

  // 各edgeにソート済みデータを蓄積
  let _ = logic.apply_with_edge(0, Box::new(1_u32)).expect("edge 0 a");
  let _ = logic.apply_with_edge(0, Box::new(3_u32)).expect("edge 0 b");
  let _ = logic.apply_with_edge(1, Box::new(2_u32)).expect("edge 1 a");
  let _ = logic.apply_with_edge(1, Box::new(4_u32)).expect("edge 1 b");

  logic.on_source_done().expect("source done");

  let mut results = Vec::new();
  loop {
    let drained = logic.drain_pending().expect("drain");
    if drained.is_empty() {
      break;
    }
    for value in drained {
      results.push(*value.downcast::<u32>().expect("downcast"));
    }
  }
  // drain時のソート済み順序: apply_with_edgeで1と2が既に出力済み、残りの3, 4がドレインされる
  assert_eq!(results, vec![3_u32, 4_u32]);
}

#[test]
fn merge_sorted_logic_on_restart_clears_state() {
  let mut logic = super::MergeSortedLogic::<u32> {
    fan_in:      2,
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    source_done: false,
  };

  let _ = logic.apply_with_edge(0, Box::new(1_u32)).expect("apply");
  logic.on_source_done().expect("source done");

  logic.on_restart().expect("restart");

  let drained = logic.drain_pending().expect("drain");
  assert!(drained.is_empty());
}

// --- merge_latest tests ---

#[test]
fn merge_latest_wraps_single_path_value_into_vec() {
  let values = Source::single(7_u32)
    .via(Flow::new().merge_latest(1).expect("merge_latest"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn merge_latest_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  assert!(flow.merge_latest(0).is_err());
}

#[test]
fn merge_latest_emits_latest_snapshot_on_each_update() {
  use alloc::vec;

  use super::merge_latest_definition;
  use crate::core::{DynValue, downcast_value};

  let def = merge_latest_definition::<u32>(2);
  let mut logic = def.logic;

  // edge 0: 値10 → 全スロット未充填のためemitなし
  let result = logic.apply_with_edge(0, Box::new(10_u32) as DynValue).expect("apply edge 0");
  assert!(result.is_empty());

  // edge 1: 値20 → 全スロット充填。Vec[10, 20]をemit
  let result = logic.apply_with_edge(1, Box::new(20_u32) as DynValue).expect("apply edge 1");
  assert_eq!(result.len(), 1);
  let snapshot = downcast_value::<Vec<u32>>(result.into_iter().next().unwrap()).expect("downcast");
  assert_eq!(snapshot, vec![10, 20]);

  // edge 0: 値30 → latestが[30, 20]に更新
  let result = logic.apply_with_edge(0, Box::new(30_u32) as DynValue).expect("apply edge 0 again");
  assert_eq!(result.len(), 1);
  let snapshot = downcast_value::<Vec<u32>>(result.into_iter().next().unwrap()).expect("downcast");
  assert_eq!(snapshot, vec![30, 20]);
}

// --- watch_termination tests ---

#[test]
fn watch_termination_passes_through_elements() {
  let values =
    Source::single(42_u32).via(Flow::new().watch_termination_mat(KeepLeft)).collect_values().expect("collect_values");
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn watch_termination_completes_stream_completion_handle() {
  let (graph, completion) = Flow::<u32, u32, StreamNotUsed>::new().watch_termination_mat(KeepRight).into_parts();
  // 実行前はPending
  assert_eq!(completion.poll(), Completion::Pending);

  let source_flow: Flow<u32, u32, StreamCompletion<()>> = Flow::from_graph(graph, completion.clone());
  let values = Source::single(1_u32).via(source_flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32]);

  // 実行後はReady
  assert_eq!(completion.poll(), Completion::Ready(Ok(())));
}

#[test]
fn watch_termination_mat_keeps_both() {
  let (_graph, (left, right)) = Flow::<u32, u32, StreamNotUsed>::new().watch_termination_mat(KeepBoth).into_parts();
  assert_eq!(left, StreamNotUsed::new());
  assert_eq!(right.poll(), Completion::Pending);
}

// --- UniformFanInShape tests ---

#[test]
fn uniform_fan_in_shape_creates_with_port_count() {
  let shape = UniformFanInShape::<u32, u32>::with_port_count(3);
  assert_eq!(shape.port_count(), 3);
  assert_eq!(shape.inlets().len(), 3);
}

#[test]
fn uniform_fan_in_shape_creates_from_parts() {
  use crate::core::shape::{Inlet, Outlet};
  let inlets = alloc::vec![Inlet::<u32>::new(), Inlet::<u32>::new()];
  let outlet = Outlet::<u64>::new();
  let shape = UniformFanInShape::new(inlets, outlet);
  assert_eq!(shape.port_count(), 2);
  assert_eq!(shape.inlets().len(), 2);
}

#[test]
fn uniform_fan_in_shape_zero_ports() {
  let shape = UniformFanInShape::<u32, u32>::with_port_count(0);
  assert_eq!(shape.port_count(), 0);
  assert!(shape.inlets().is_empty());
}

// take_shutdown_request の全フラグクリアを検証するためのカスタム FlowLogic
struct ShutdownFlagFlowLogic {
  shutdown_requested: bool,
}

impl FlowLogic for ShutdownFlagFlowLogic {
  fn apply(&mut self, input: DynValue) -> Result<alloc::vec::Vec<DynValue>, StreamError> {
    Ok(alloc::vec![input])
  }

  fn take_shutdown_request(&mut self) -> bool {
    let was_requested = self.shutdown_requested;
    self.shutdown_requested = false;
    was_requested
  }
}

struct MultiOutputRetryTestLogic {
  restart_calls: ArcShared<SpinSyncMutex<u32>>,
}

impl FlowLogic for MultiOutputRetryTestLogic {
  fn apply(&mut self, input: DynValue) -> Result<alloc::vec::Vec<DynValue>, StreamError> {
    let value = *input.downcast::<u32>().map_err(|_| StreamError::TypeMismatch)?;
    Ok(alloc::vec![Box::new(value.saturating_add(1)) as DynValue, Box::new(value.saturating_add(2)) as DynValue,])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    let mut guard = self.restart_calls.lock();
    *guard = guard.saturating_add(1);
    Ok(())
  }
}

#[test]
fn flow_lazy_flow_take_shutdown_request_clears_all_inner_flags() {
  // Given: 3つの inner logic すべてにシャットダウンフラグが設定された LazyFlowLogic
  let mut logic = super::LazyFlowLogic::<u32, u32, StreamNotUsed, fn() -> Flow<u32, u32, StreamNotUsed>> {
    factory:      None,
    inner_logics: alloc::vec![
      Box::new(ShutdownFlagFlowLogic { shutdown_requested: true }),
      Box::new(ShutdownFlagFlowLogic { shutdown_requested: true }),
      Box::new(ShutdownFlagFlowLogic { shutdown_requested: true }),
    ],
    mat:          None,
    _pd:          PhantomData,
  };

  // When: take_shutdown_request を呼ぶ
  let first_call = logic.take_shutdown_request();

  // Then: true が返る（いずれかのフラグが設定されていたため）
  assert!(first_call);

  // When: 再度呼ぶ（fold で全フラグがクリア済みであれば false になる）
  let second_call = logic.take_shutdown_request();

  // Then: 全フラグがクリアされているため false
  // any() 短絡評価の場合、2番目以降のフラグが未クリアで true になりテスト失敗
  assert!(!second_call);
}

#[test]
fn retry_flow_logic_queues_multiple_retries_before_restarting_inner() {
  let restart_calls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let mut logic = super::RetryFlowLogic::<u32, u32, _>::new(
    alloc::vec![Box::new(MultiOutputRetryTestLogic { restart_calls: restart_calls.clone() })],
    |input: &u32, output: &u32| {
      if *input < 10 { Some(output.saturating_add(100)) } else { None }
    },
    4,
    0,
    0,
    0,
  );

  logic.on_tick(0).expect("tick");
  let first_outputs = logic.apply(Box::new(1_u32)).expect("apply");
  assert!(first_outputs.is_empty());
  assert_eq!(*restart_calls.lock(), 1_u32);
  assert!(logic.has_pending_output());

  let first_retry = logic.drain_pending().expect("first retry");
  let first_values: alloc::vec::Vec<u32> =
    first_retry.into_iter().map(|value| *value.downcast::<u32>().expect("u32 output")).collect();
  assert_eq!(first_values, vec![103_u32, 104_u32]);
  assert!(logic.has_pending_output());

  let second_retry = logic.drain_pending().expect("second retry");
  let second_values: alloc::vec::Vec<u32> =
    second_retry.into_iter().map(|value| *value.downcast::<u32>().expect("u32 output")).collect();
  assert_eq!(second_values, vec![104_u32, 105_u32]);
  assert!(!logic.has_pending_output());
}

#[test]
fn throttle_enforcing_mode_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().throttle(2, crate::core::ThrottleMode::Enforcing).expect("throttle"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn throttle_enforcing_mode_fails_on_capacity_overflow() {
  // map_concat は1入力から複数出力を生成し、スロットルの内部バッファを
  // 下流が排出できるより速く飽和させる。
  let result = Source::single(alloc::vec![1_u32, 2, 3])
    .via(Flow::new().map_concat(|v: alloc::vec::Vec<u32>| v))
    .via(Flow::new().throttle(1, crate::core::ThrottleMode::Enforcing).expect("throttle"))
    .collect_values();
  assert_eq!(result, Err(StreamError::BufferOverflow));
}

#[test]
fn throttle_enforcing_logic_returns_buffer_overflow_when_pending_full() {
  use super::AsyncBoundaryLogic;

  let mut logic = AsyncBoundaryLogic::<u32> { pending: VecDeque::new(), capacity: 1, enforcing: true };

  // 最初の要素はキャパシティ内に収まる
  let first: DynValue = Box::new(1_u32);
  assert!(logic.apply(first).is_ok());
  assert_eq!(logic.pending.len(), 1);

  // enforcing モードではキャパシティに達しても can_accept_input は true のまま
  assert!(logic.can_accept_input());

  // 2番目の要素で BufferOverflow が発生する
  let second: DynValue = Box::new(2_u32);
  assert!(matches!(logic.apply(second), Err(StreamError::BufferOverflow)));
}

#[test]
fn throttle_shaping_logic_uses_backpressure_at_capacity() {
  use super::AsyncBoundaryLogic;

  let mut logic = AsyncBoundaryLogic::<u32> { pending: VecDeque::new(), capacity: 1, enforcing: false };

  let first: DynValue = Box::new(1_u32);
  assert!(logic.apply(first).is_ok());
  assert_eq!(logic.pending.len(), 1);

  // shaping モードではキャパシティに達すると入力を拒否する（バックプレッシャー）
  assert!(!logic.can_accept_input());
}

#[test]
fn buffer_logic_drop_buffer_clears_pending_and_keeps_newest() {
  let mut logic = super::BufferLogic::<u32> {
    capacity:          2,
    overflow_strategy: OverflowStrategy::DropBuffer,
    pending:           VecDeque::new(),
    source_done:       false,
  };

  let _ = logic.apply(Box::new(1_u32)).expect("first apply");
  let _ = logic.apply(Box::new(2_u32)).expect("second apply");
  let _ = logic.apply(Box::new(3_u32)).expect("third apply");
  logic.on_source_done().expect("source done");

  let drained = logic.drain_pending().expect("drain");
  let drained_values: Vec<u32> = drained.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(drained_values, vec![3_u32]);
  assert!(logic.drain_pending().expect("drain empty").is_empty());
}

#[test]
fn buffer_logic_fail_returns_buffer_overflow_when_full() {
  let mut logic = super::BufferLogic::<u32> {
    capacity:          1,
    overflow_strategy: OverflowStrategy::Fail,
    pending:           VecDeque::new(),
    source_done:       false,
  };

  let _ = logic.apply(Box::new(1_u32)).expect("first apply");
  let result = logic.apply(Box::new(2_u32));
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn distinct_removes_duplicate_elements() {
  let values =
    Source::from_array([1_u32, 2, 1, 3, 2, 3, 4]).via(Flow::new().distinct()).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3, 4]);
}

#[test]
fn distinct_on_already_unique_passes_all() {
  let values = Source::from_array([1_u32, 2, 3]).via(Flow::new().distinct()).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn distinct_by_removes_elements_with_duplicate_key() {
  let values = Source::from_array([(1_u32, "a"), (2, "b"), (1, "c"), (3, "d")])
    .via(Flow::new().distinct_by(|pair: &(u32, &str)| pair.0))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![(1_u32, "a"), (2, "b"), (3, "d")]);
}

#[test]
fn from_graph_creates_flow_from_existing_graph() {
  let original = Flow::<u32, u32, StreamNotUsed>::new().map(|x| x * 2);
  let (graph, mat) = original.into_parts();
  let reconstructed = Flow::<u32, u32, StreamNotUsed>::from_graph(graph, mat);
  let values = Source::from_array([1_u32, 2, 3]).via(reconstructed).collect_values().expect("collect_values");
  assert_eq!(values, vec![2_u32, 4, 6]);
}

#[test]
fn debounce_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().debounce(2).expect("debounce")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn debounce_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.debounce(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn sample_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().sample(2).expect("sample")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn sample_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.sample(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn flow_named_keeps_elements_and_sets_attributes() {
  let values = Source::single(7_u32).via(Flow::new().named("test-flow")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);

  let (graph, _mat) = Flow::<u32, u32, StreamNotUsed>::new().named("test-flow").into_parts();
  assert_eq!(graph.attributes().names(), &[alloc::string::String::from("test-flow")]);
}

#[test]
fn flow_with_and_add_attributes_merge_names() {
  let (graph, _mat) = Flow::<u32, u32, StreamNotUsed>::new()
    .with_attributes(crate::core::attributes::Attributes::named("base"))
    .add_attributes(crate::core::attributes::Attributes::named("extra"))
    .into_parts();
  assert_eq!(graph.attributes().names(), &[alloc::string::String::from("base"), alloc::string::String::from("extra")]);
}

#[test]
fn flow_from_materializer_creates_flow() {
  let flow = Flow::from_materializer(|| Flow::from_function(|x: u32| x * 2));
  let values = Source::single(5_u32).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![10_u32]);
}

#[test]
fn flow_as_flow_with_context_returns_wrapper() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let fwc = flow.into_flow_with_context();
  let _ = fwc.into_flow();
}

// --- B5: Flow.fromSinkAndSourceMat ---

#[test]
fn flow_from_sink_and_source_mat_keeps_both_materialized_values() {
  // Given: a sink and source with distinct materialized values
  use crate::core::materialization::KeepBoth;

  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 42_u32);
  let source = Source::single(1_u64).map_materialized_value(|_| "hello");

  // When: creating a flow with KeepBoth
  let flow = Flow::from_sink_and_source_mat(sink, source, KeepBoth);

  // Then: the materialized value is a tuple of both
  let (_graph, mat) = flow.into_parts();
  let (left, right) = mat;
  assert_eq!(left, 42_u32);
  assert_eq!(right, "hello");
}

#[test]
fn flow_from_sink_and_source_mat_keeps_left_materialized_value() {
  // Given: a sink and source with distinct materialized values
  use crate::core::materialization::KeepLeft;

  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 42_u32);
  let source = Source::single(1_u64).map_materialized_value(|_| "hello");

  // When: creating a flow with KeepLeft
  let flow = Flow::from_sink_and_source_mat(sink, source, KeepLeft);

  // Then: only the left (sink) materialized value is kept
  let (_graph, mat) = flow.into_parts();
  assert_eq!(mat, 42_u32);
}

#[test]
fn flow_from_sink_and_source_mat_keeps_right_materialized_value() {
  // Given: a sink and source with distinct materialized values
  use crate::core::materialization::KeepRight;

  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 42_u32);
  let source = Source::single(1_u64).map_materialized_value(|_| "hello");

  // When: creating a flow with KeepRight
  let flow = Flow::from_sink_and_source_mat(sink, source, KeepRight);

  // Then: only the right (source) materialized value is kept
  let (_graph, mat) = flow.into_parts();
  assert_eq!(mat, "hello");
}

// --- B6: Flow.fromSinkAndSourceCoupledMat ---

#[test]
fn flow_from_sink_and_source_coupled_mat_keeps_both_materialized_values() {
  // Given: a sink and source with distinct materialized values
  use crate::core::materialization::KeepBoth;

  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let source = Source::single(1_u64).map_materialized_value(|_| true);

  // When: creating a coupled flow with KeepBoth
  let flow = Flow::from_sink_and_source_coupled_mat(sink, source, KeepBoth);

  // Then: the materialized value is a tuple of both
  let (_graph, mat) = flow.into_parts();
  let (left, right) = mat;
  assert_eq!(left, 99_i32);
  assert_eq!(right, true);
}

#[test]
fn flow_from_sink_and_source_coupled_mat_keeps_left_materialized_value() {
  // Given: a sink and source with distinct materialized values
  use crate::core::materialization::KeepLeft;

  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let source = Source::single(1_u64).map_materialized_value(|_| true);

  // When: creating a coupled flow with KeepLeft
  let flow = Flow::from_sink_and_source_coupled_mat(sink, source, KeepLeft);

  // Then: only the left (sink) materialized value is kept
  let (_graph, mat) = flow.into_parts();
  assert_eq!(mat, 99_i32);
}

#[test]
fn flow_from_sink_and_source_coupled_mat_completes_wrapped_sink_when_source_finishes() {
  use alloc::sync::Arc;
  use std::sync::atomic::{AtomicBool, Ordering};

  let sink_completed = Arc::new(AtomicBool::new(false));
  let sink = Sink::<u32, _>::on_complete({
    let sink_completed = sink_completed.clone();
    move |_| sink_completed.store(true, Ordering::SeqCst)
  });
  let source = Source::<u32, _>::empty().watch_termination_mat(KeepRight);

  let flow = Flow::from_sink_and_source_coupled_mat(sink, source, KeepRight);
  let (_graph, right_completion) = flow.into_parts();
  let source_flow: Flow<u32, u32, StreamCompletion<()>> = Flow::from_graph(_graph, right_completion.clone());

  let values = Source::single(1_u32).via(source_flow).collect_values().expect("collect_values");

  assert!(values.is_empty());
  assert!(sink_completed.load(Ordering::SeqCst));
  assert_eq!(right_completion.poll(), Completion::Ready(Ok(())));
}

#[test]
fn flow_from_sink_and_source_coupled_mat_cancels_wrapped_source_when_sink_cancels() {
  let source = Source::<u32, _>::never().watch_termination_mat(KeepRight);
  let sink = Sink::<u32, _>::cancelled();

  let flow = Flow::from_sink_and_source_coupled_mat(sink, source, KeepRight);
  let (_graph, right_completion) = flow.into_parts();
  let source_flow: Flow<u32, u32, StreamCompletion<()>> = Flow::from_graph(_graph, right_completion.clone());

  let values = Source::single(1_u32).via(source_flow).collect_values().expect("collect_values");

  assert!(values.is_empty());
  assert_eq!(right_completion.poll(), Completion::Ready(Ok(())));
}

// --- A4: group_by with SubstreamCancelStrategy ---

#[test]
fn flow_group_by_with_propagate_strategy_creates_subflow() {
  // Given: a flow with group_by using Propagate strategy
  use crate::core::SubstreamCancelStrategy;

  let flow = Flow::<u32, u32, StreamNotUsed>::new();

  // When: calling group_by with SubstreamCancelStrategy
  let result = flow.group_by(10, |x| x % 2, SubstreamCancelStrategy::Propagate);

  // Then: the subflow is created successfully
  assert!(result.is_ok());
}

#[test]
fn flow_group_by_with_drain_strategy_creates_subflow() {
  // Given: a flow with group_by using Drain strategy
  use crate::core::SubstreamCancelStrategy;

  let flow = Flow::<u32, u32, StreamNotUsed>::new();

  // When: calling group_by with Drain strategy
  let result = flow.group_by(10, |x| x % 2, SubstreamCancelStrategy::Drain);

  // Then: the subflow is created successfully
  assert!(result.is_ok());
}

// --- r#async() ---

#[test]
fn flow_async_passes_single_element_through() {
  // Given: a source emitting a single element
  // When: passing through a flow with an async boundary
  let values = Source::single(7_u32).via(Flow::new().r#async()).collect_values().expect("collect_values");

  // Then: the element is forwarded unchanged
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn flow_async_passes_multiple_elements_through() {
  // Given: a source emitting multiple elements
  let source = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]));

  // When: passing through a flow with an async boundary
  let values = source.via(Flow::new().r#async()).collect_values().expect("collect_values");

  // Then: all elements are forwarded in order
  assert_eq!(values, vec![1_u32, 2, 3, 4, 5]);
}

#[test]
fn flow_async_preserves_element_order() {
  // Given: a source emitting a descending sequence
  let source = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[5, 4, 3, 2, 1]));

  // When: passing through an async boundary
  let values = source.via(Flow::new().r#async()).collect_values().expect("collect_values");

  // Then: elements arrive in original order
  assert_eq!(values, vec![5_u32, 4, 3, 2, 1]);
}

#[test]
fn flow_async_handles_empty_source() {
  // Given: an empty source
  let source = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]));

  // When: passing through an async boundary
  let values = source.via(Flow::new().r#async()).collect_values().expect("collect_values");

  // Then: no elements are emitted, stream completes normally
  assert!(values.is_empty());
}

#[test]
fn flow_async_composes_with_map() {
  // Given: a source with map + async boundary
  let source = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]));

  // When: map is applied before async boundary
  let values = source.via(Flow::new().map(|x: u32| x * 10).r#async()).collect_values().expect("collect_values");

  // Then: map is applied and elements pass through the boundary
  assert_eq!(values, vec![10_u32, 20, 30]);
}

#[test]
fn flow_async_composes_with_map_after() {
  // Given: a source with async boundary followed by map
  let source = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]));

  // When: async boundary is applied before map
  let values =
    source.via(Flow::new().r#async()).via(Flow::new().map(|x: u32| x + 100)).collect_values().expect("collect_values");

  // Then: both stages work correctly
  assert_eq!(values, vec![101_u32, 102, 103]);
}

#[test]
fn flow_async_chained_multiple_boundaries() {
  // Given: a source with multiple chained async boundaries
  let source = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]));

  // When: two async boundaries are chained
  let values = source.via(Flow::new().r#async().r#async()).collect_values().expect("collect_values");

  // Then: elements pass through both boundaries correctly
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn flow_async_propagates_upstream_error() {
  // Given: a source that emits an error mid-stream
  let source = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1), Err(StreamError::Failed)]),
  );

  // When: passing through an async boundary
  let result = source.via(Flow::new().r#async()).collect_values();

  // Then: the error propagates through the boundary
  assert!(result.is_err());
}

#[test]
fn flow_async_with_filter_composition() {
  // Given: a source filtered then async-boundaried
  let source = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5, 6]));

  // When: filter even numbers, then async boundary
  let values = source.via(Flow::new().filter(|x: &u32| x % 2 == 0).r#async()).collect_values().expect("collect_values");

  // Then: only even numbers pass through
  assert_eq!(values, vec![2_u32, 4, 6]);
}

// --- B-1: r#async() per-node attribute propagation ---

#[test]
fn flow_async_marks_last_node_with_async_attribute_in_plan() {
  // Given: a source → map.async() → sink pipeline
  let source = Source::single(1_u32);
  let flow = Flow::new().map(|x: u32| x + 1).r#async();

  // When: building a complete pipeline and converting to plan
  let (mut graph, _) = source.via(flow).into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // Then: the map/async flow stage has async boundary attribute
  let has_async_stage = plan.stages.iter().any(|s| s.attributes().is_async());
  assert!(has_async_stage, "at least one stage should have async boundary attribute");
}

#[test]
fn flow_async_does_not_set_async_on_preceding_stages() {
  // Given: source → map.async() → filter → sink
  let source = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]));
  let flow = Flow::new().map(|x: u32| x + 1).r#async();
  let flow2 = Flow::new().filter(|x: &u32| *x > 0);

  // When: building a complete pipeline
  let (mut graph, _) = source.via(flow).via(flow2).into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // Then: only the async-marked stage has async attribute; source and filter do not
  let async_count = plan.stages.iter().filter(|s| s.attributes().is_async()).count();
  assert_eq!(async_count, 1, "exactly one stage should have async boundary");
}

#[test]
fn flow_async_with_dispatcher_marks_node_with_both_attributes() {
  // 準備: source → flow.async_with_dispatcher("custom") → sink
  let source = Source::single(1_u32);
  let flow = Flow::new().map(|x: u32| x).async_with_dispatcher("custom-dispatcher");

  // 実行: パイプラインを構築しプランに変換
  let (mut graph, _) = source.via(flow).into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // 検証: async + dispatcher 属性が付いた stage を特定し、他の stage には付いていないことを確認
  let async_indices: Vec<usize> =
    plan.stages.iter().enumerate().filter(|(_, s)| s.attributes().is_async()).map(|(i, _)| i).collect();
  assert_eq!(async_indices.len(), 1, "async 属性は 1 つの stage のみに付くべき");

  let async_stage = &plan.stages[async_indices[0]];
  let dispatcher = async_stage.attributes().get::<crate::core::attributes::DispatcherAttribute>();
  assert!(dispatcher.is_some(), "async stage に DispatcherAttribute がない");
  assert_eq!(dispatcher.unwrap().name(), "custom-dispatcher");

  // async でない stage に dispatcher が付いていないことを確認
  for (i, stage) in plan.stages.iter().enumerate() {
    if i != async_indices[0] {
      assert!(!stage.attributes().is_async(), "stage {i} に意図しない async 属性が付いている");
    }
  }
}

#[test]
fn flow_async_attribute_survives_via_composition() {
  // Given: two flows composed with .via(), first has async
  let source = Source::single(1_u32);
  let flow1 = Flow::new().map(|x: u32| x * 2).r#async();
  let flow2 = Flow::new().map(|x: u32| x + 1);

  // When: composing flow1.via(flow2) through source with a sink
  let (mut graph, _) = source.via(flow1).via(flow2).into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // Then: the async-marked stage's attribute persists; flow2's stage is NOT async
  let async_count = plan.stages.iter().filter(|s| s.attributes().is_async()).count();
  assert_eq!(async_count, 1);
}

#[test]
fn multiple_flow_async_marks_multiple_stages() {
  // Given: source → flow1.async() → flow2.async() → sink
  let source = Source::single(1_u32);
  let flow1 = Flow::new().map(|x: u32| x * 2).r#async();
  let flow2 = Flow::new().map(|x: u32| x + 1).r#async();

  // When: building a complete pipeline
  let (mut graph, _) = source.via(flow1).via(flow2).into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // Then: two stages have async boundary
  let async_count = plan.stages.iter().filter(|s| s.attributes().is_async()).count();
  assert_eq!(async_count, 2);
}

// --- fold_while tests ---

#[test]
fn fold_while_accumulates_while_predicate_holds() {
  // Given: a stream of [1, 2, 3, 4]
  // When: fold_while accumulates while acc < 6
  let values = Source::from_array([1_u32, 2, 3, 4])
    .via(Flow::new().fold_while(0_u32, |acc, _| *acc < 6, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");

  // Then: accumulation stops updating at 6 (1+2+3=6, predicate false for 4)
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32, 6_u32]);
}

#[test]
fn fold_while_with_always_true_predicate_behaves_like_fold() {
  // Given: a stream of [1, 2, 3]
  // When: fold_while with always-true predicate
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().fold_while(0_u32, |_, _| true, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");

  // Then: behaves identically to fold
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn fold_while_with_immediately_false_predicate_emits_initial_unchanged() {
  // Given: a stream of [10, 20, 30]
  // When: fold_while with always-false predicate
  let values = Source::from_array([10_u32, 20, 30])
    .via(Flow::new().fold_while(0_u32, |_, _| false, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");

  // Then: accumulator never updates, emits initial value for each element
  assert_eq!(values, vec![0_u32, 0_u32, 0_u32]);
}

#[test]
fn fold_while_on_empty_stream_emits_nothing() {
  // Given: an empty stream
  // When: fold_while is applied
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().fold_while(99_u32, |_, _| true, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");

  // Then: no output (same as fold on empty stream)
  assert!(values.is_empty());
}

// --- compression overload tests ---

#[test]
#[cfg(feature = "compression")]
fn deflate_with_level_round_trips_through_inflate() {
  // Given: a payload compressed with deflate at level 9
  let payload = b"deflate-level-nine-payload".to_vec();

  // When: deflate_with_level(9) then inflate
  let values = Source::single(payload.clone())
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().deflate_with_level(9))
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().inflate())
    .collect_values()
    .expect("collect_values");

  // Then: original payload is recovered
  assert_eq!(values, vec![payload]);
}

#[test]
#[cfg(feature = "compression")]
fn deflate_with_options_nowrap_round_trips() {
  // Given: a payload compressed with deflate level 6 nowrap=true
  let payload = b"deflate-nowrap-options-test".to_vec();

  // When: deflate_with_options(6, true) then inflate_with_options(_, true)
  let values = Source::single(payload.clone())
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().deflate_with_options(6, true))
    .via(
      Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().inflate_with_options(FLOW_DECOMPRESSION_MAX_BYTES_DEFAULT, true),
    )
    .collect_values()
    .expect("collect_values");

  // Then: original payload is recovered
  assert_eq!(values, vec![payload]);
}

#[test]
#[cfg(feature = "compression")]
fn gzip_with_level_round_trips_through_gzip_decompress() {
  // Given: a payload compressed with gzip at level 1
  let payload = b"gzip-level-one-fast-compress".to_vec();

  // When: gzip_with_level(1) then gzip_decompress
  let values = Source::single(payload.clone())
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip_with_level(1))
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip_decompress())
    .collect_values()
    .expect("collect_values");

  // Then: original payload is recovered
  assert_eq!(values, vec![payload]);
}

#[test]
#[cfg(feature = "compression")]
fn gzip_decompress_with_max_bytes_rejects_oversized_payload() {
  // Given: a payload larger than a small max_bytes limit
  let payload = vec![0xaa_u8; 256];
  let encoded = Source::single(payload)
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip())
    .collect_values()
    .expect("collect_values")
    .pop()
    .expect("encoded payload");

  // When: decompressing with max_bytes = 64
  let result = Source::single(encoded)
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().gzip_decompress_with_max_bytes(64))
    .collect_values();

  // Then: compression error
  assert!(matches!(result, Err(StreamError::CompressionError { .. })));
}

#[test]
#[cfg(feature = "compression")]
fn inflate_with_max_bytes_rejects_oversized_payload() {
  // Given: a payload larger than a small max_bytes limit
  let payload = vec![0xbb_u8; 256];
  let encoded = miniz_oxide::deflate::compress_to_vec(&payload, 6);

  // When: inflating with max_bytes = 64
  let result = Source::single(encoded)
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().inflate_with_max_bytes(64))
    .collect_values();

  // Then: compression error
  assert!(matches!(result, Err(StreamError::CompressionError { .. })));
}

#[test]
#[cfg(feature = "compression")]
fn inflate_with_options_nowrap_false_decompresses_zlib_wrapped() {
  // Given: a zlib-wrapped (nowrap=false) deflate payload
  let payload = b"inflate-zlib-wrapped-test".to_vec();

  // When: deflate_with_options(6, false) then inflate_with_options(_, false)
  let values = Source::single(payload.clone())
    .via(Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().deflate_with_options(6, false))
    .via(
      Flow::<Vec<u8>, Vec<u8>, StreamNotUsed>::new().inflate_with_options(FLOW_DECOMPRESSION_MAX_BYTES_DEFAULT, false),
    )
    .collect_values()
    .expect("collect_values");

  // Then: original payload is recovered
  assert_eq!(values, vec![payload]);
}

// ---------------------------------------------------------------------------
// Flow::from_graph_stage — end-to-end integration tests
// ---------------------------------------------------------------------------

use crate::core::{
  graph::{GraphStage, GraphStageLogic},
  shape::{Inlet, Outlet, StreamShape},
  stage::StageContext,
};

/// A simple map-like GraphStageLogic that adds 100 to each input.
struct AddHundredLogic;

impl GraphStageLogic<u32, u32, StreamNotUsed> for AddHundredLogic {
  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    ctx.push(value + 100);
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

struct AddHundredStage;

impl GraphStage<u32, u32, StreamNotUsed> for AddHundredStage {
  fn shape(&self) -> StreamShape<u32, u32> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> {
    Box::new(AddHundredLogic)
  }
}

/// A GraphStageLogic that filters, passing only values > threshold.
struct ThresholdFilterLogic {
  threshold: u32,
}

impl GraphStageLogic<u32, u32, StreamNotUsed> for ThresholdFilterLogic {
  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    if value > self.threshold {
      ctx.push(value);
    }
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

struct ThresholdFilterStage {
  threshold: u32,
}

impl GraphStage<u32, u32, StreamNotUsed> for ThresholdFilterStage {
  fn shape(&self) -> StreamShape<u32, u32> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> {
    Box::new(ThresholdFilterLogic { threshold: self.threshold })
  }
}

/// A GraphStageLogic with a materialized value (counter of processed elements).
struct CountingLogic {
  count: ArcShared<SpinSyncMutex<usize>>,
}

impl GraphStageLogic<u32, u32, ArcShared<SpinSyncMutex<usize>>> for CountingLogic {
  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    *self.count.lock() += 1;
    ctx.push(value);
  }

  fn materialized(&mut self) -> ArcShared<SpinSyncMutex<usize>> {
    self.count.clone()
  }
}

struct CountingStage {
  count: ArcShared<SpinSyncMutex<usize>>,
}

impl CountingStage {
  fn new() -> Self {
    Self { count: ArcShared::new(SpinSyncMutex::new(0)) }
  }
}

impl GraphStage<u32, u32, ArcShared<SpinSyncMutex<usize>>> for CountingStage {
  fn shape(&self) -> StreamShape<u32, u32> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<u32, u32, ArcShared<SpinSyncMutex<usize>>> + Send> {
    Box::new(CountingLogic { count: self.count.clone() })
  }
}

#[test]
fn from_graph_stage_map_like_stage_produces_correct_values() {
  // Given: a custom GraphStage that adds 100
  let flow = Flow::<u32, u32, StreamNotUsed>::from_graph_stage(AddHundredStage);

  // When: running through a pipeline
  let values = Source::from_array([1_u32, 2, 3]).via(flow).collect_values().expect("collect_values");

  // Then: each value has 100 added
  assert_eq!(values, alloc::vec![101_u32, 102, 103]);
}

#[test]
fn from_graph_stage_filter_like_stage_drops_elements() {
  // Given: a custom GraphStage that filters values > 3
  let flow = Flow::<u32, u32, StreamNotUsed>::from_graph_stage(ThresholdFilterStage { threshold: 3 });

  // When: running through a pipeline
  let values = Source::from_array([1_u32, 2, 3, 4, 5]).via(flow).collect_values().expect("collect_values");

  // Then: only values > 3 pass through
  assert_eq!(values, alloc::vec![4_u32, 5]);
}

#[test]
fn from_graph_stage_chained_with_standard_flow_operators() {
  // Given: a custom stage chained with a standard map
  let custom_flow = Flow::<u32, u32, StreamNotUsed>::from_graph_stage(AddHundredStage);
  let combined = custom_flow.via(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 2));

  // When: running through a pipeline
  let values = Source::from_array([1_u32, 2]).via(combined).collect_values().expect("collect_values");

  // Then: (1+100)*2=202, (2+100)*2=204
  assert_eq!(values, alloc::vec![202_u32, 204]);
}

#[test]
fn from_graph_stage_with_materialized_value() {
  // Given: a counting stage with a shared counter
  let stage = CountingStage::new();
  let counter = stage.count.clone();
  let flow = Flow::<u32, u32, ArcShared<SpinSyncMutex<usize>>>::from_graph_stage(stage);

  // When: running through a pipeline
  let values = Source::from_array([10_u32, 20, 30]).via(flow).collect_values().expect("collect_values");

  // Then: all values pass through and the counter reflects the count
  assert_eq!(values, alloc::vec![10_u32, 20, 30]);
  assert_eq!(*counter.lock(), 3);
}

#[test]
fn from_graph_stage_empty_source_produces_no_output() {
  // Given: a custom stage with an empty source
  let flow = Flow::<u32, u32, StreamNotUsed>::from_graph_stage(AddHundredStage);

  // When: running with an empty source
  let values = Source::<u32, StreamNotUsed>::empty().via(flow).collect_values().expect("collect_values");

  // Then: no output
  assert!(values.is_empty());
}
