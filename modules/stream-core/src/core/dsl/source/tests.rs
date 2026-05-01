use alloc::{boxed::Box, collections::VecDeque, string::String, vec::Vec};
use core::{
  future::{Future, ready},
  marker::PhantomData,
  pin::{Pin, pin},
  task::{Context, Poll, Waker},
};
use std::{
  env,
  panic::AssertUnwindSafe,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread::{self, Builder},
  time::{Duration, Instant},
};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{
  CycleSourceLogic, EmptySourceLogic, IterateSourceLogic, LazySourceLogic, QueueSourceLogic,
  QueueWithOverflowSourceLogic, RepeatSourceLogic, StreamGraph, UnboundedQueueSourceLogic,
};
use crate::core::{
  BoundedSourceQueue, DynValue, OverflowStrategy, QueueOfferResult, RestartConfig, SharedKillSwitch, SourceLogic,
  StageDefinition, StreamDslError, StreamError, SubstreamCancelStrategy, ThrottleMode,
  attributes::{Attributes, DispatcherAttribute},
  dsl::{RunnableGraph, Sink, Source, tests::RunWithCollectSink},
  r#impl::{
    fusing::StreamBufferConfig,
    materialization::{Stream, StreamShared, StreamState},
    queue::{SourceQueue, SourceQueueWithComplete},
  },
  materialization::{
    Completion, DriveOutcome, KeepBoth, KeepLeft, KeepRight, Materialized, Materializer, StreamCompletion, StreamDone,
    StreamNotUsed,
  },
  stage::StageKind,
  validate_positive_argument,
};

struct CreateSourceTestLogic<T, F> {
  queue:    BoundedSourceQueue<T>,
  producer: Option<F>,
}

impl<T, F> CreateSourceTestLogic<T, F> {
  const fn new(queue: BoundedSourceQueue<T>, producer: F) -> Self {
    Self { queue, producer: Some(producer) }
  }

  fn start_producer_if_needed(&mut self) -> Result<(), StreamError>
  where
    T: Send + Sync + 'static,
    F: FnOnce(BoundedSourceQueue<T>) + Send + 'static, {
    let Some(producer) = self.producer.take() else {
      return Ok(());
    };

    let producer_queue = self.queue.clone();
    let termination_queue = self.queue.clone();
    let spawn_result = Builder::new().name("fraktor-streams-create".to_string()).spawn(move || {
      let result = std::panic::catch_unwind(AssertUnwindSafe(|| producer(producer_queue)));
      match result {
        | Ok(()) => {
          let _ = termination_queue.complete_if_open();
        },
        | Err(_) => {
          let _ = termination_queue.fail_if_open(StreamError::Failed);
        },
      }
    });

    if spawn_result.is_err() {
      let _ = self.queue.fail_if_open(StreamError::Failed);
      return Err(StreamError::Failed);
    }

    Ok(())
  }
}

impl<T, F> SourceLogic for CreateSourceTestLogic<T, F>
where
  T: Send + Sync + 'static,
  F: FnOnce(BoundedSourceQueue<T>) + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    self.start_producer_if_needed()?;
    match self.queue.poll_or_drain()? {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Ok(None),
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.queue.close_for_cancel();
    Ok(())
  }
}

impl<Out> Source<Out, StreamNotUsed>
where
  Out: Send + Sync + 'static,
{
  /// Construct a source from a background producer for unit tests.
  pub fn create<F>(capacity: usize, producer: F) -> Result<Source<Out, BoundedSourceQueue<Out>>, StreamDslError>
  where
    F: FnOnce(BoundedSourceQueue<Out>) + Send + 'static, {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let queue = BoundedSourceQueue::new(capacity, OverflowStrategy::Backpressure);
    let logic = CreateSourceTestLogic::new(queue.clone(), producer);
    Ok(Source::from_logic(StageKind::Custom, logic).map_materialized_value(move |_| queue))
  }
}

#[test]
fn empty_source_logic_drains_on_shutdown() {
  let logic = EmptySourceLogic;

  assert!(logic.should_drain_on_shutdown());
}

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
  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    self.calls += 1;
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let stream = StreamShared::new(stream);
    Ok(Materialized::new(stream, materialized))
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
}

impl<T: Unpin> Future for YieldThenOutputFuture<T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    if this.poll_count < this.ready_after {
      this.poll_count = this.poll_count.saturating_add(1);
      Poll::Pending
    } else {
      Poll::Ready(this.value.take().expect("future value"))
    }
  }
}

struct NeverReadyFuture<T> {
  _pd: PhantomData<fn() -> T>,
}

impl<T> NeverReadyFuture<T> {
  const fn new() -> Self {
    Self { _pd: PhantomData }
  }
}

impl<T> Future for NeverReadyFuture<T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
    Poll::Pending
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

impl SourceLogic for FailureSequenceSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.steps.pop_front() {
      | Some(Ok(value)) => Ok(Some(Box::new(value) as DynValue)),
      | Some(Err(error)) => Err(error),
      | None => Ok(None),
    }
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

fn poll_ready<F>(future: F) -> F::Output
where
  F: Future, {
  let mut future = pin!(future);
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);
  match future.as_mut().poll(&mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future should be ready"),
  }
}

fn noop_waker() -> Waker {
  Waker::noop().clone()
}

/// Base loop count for tests that must wait for a background producer thread to be
/// scheduled and execute.  The actual count is multiplied by `TEST_TIME_FACTOR`
/// (default 1.0, set to 2.0 in CI) so the value needs to be large enough to
/// tolerate OS-level thread scheduling delays in resource-constrained CI environments
/// while remaining fast on local machines where threads are scheduled promptly.
const THREAD_SYNC_ATTEMPTS: usize = 4096;

fn test_time_factor() -> f64 {
  match env::var("TEST_TIME_FACTOR") {
    | Err(_) => 1.0,
    | Ok(raw) => {
      let factor = raw
        .parse::<f64>()
        .unwrap_or_else(|e| panic!("test_time_factor: TEST_TIME_FACTOR={raw:?} is not a valid f64: {e}"));
      assert!(factor > 0.0, "test_time_factor: TEST_TIME_FACTOR={raw:?} must be positive, got {factor}");
      factor
    },
  }
}

fn scaled_attempts(base: usize) -> usize {
  ((base as f64) * test_time_factor()).ceil() as usize
}

fn scaled_duration(base: Duration) -> Duration {
  base.mul_f64(test_time_factor())
}

fn drive_materialized_completion<T>(materialized: &Materialized<StreamCompletion<T>>) -> Completion<T>
where
  T: Clone, {
  for _ in 0..64 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }
  materialized.materialized().poll()
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
fn source_run_fold_accumulates_values_via_sink_shortcut() {
  let mut materializer = RecordingMaterializer::default();
  let materialized =
    Source::from_array([1_u32, 2, 3]).run_fold(0_u32, |acc, value| acc + value, &mut materializer).expect("run_fold");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(6_u32)));
}

#[test]
fn source_run_fold_async_accumulates_values_when_future_is_ready() {
  let mut materializer = RecordingMaterializer::default();
  let materialized = Source::from_array([1_u32, 2, 3])
    .run_fold_async(0_u32, |acc, value| ready(acc + value), &mut materializer)
    .expect("run_fold_async");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(6_u32)));
}

#[test]
fn source_run_fold_async_waits_for_pending_future_before_completion() {
  let mut materializer = RecordingMaterializer::default();
  let materialized = Source::single(7_u32)
    .run_fold_async(0_u32, |acc, value| YieldThenOutputFuture::new(acc + value), &mut materializer)
    .expect("run_fold_async");

  assert_eq!(materializer.calls, 1);
  assert_eq!(materialized.stream().drive(), DriveOutcome::Progressed);
  assert_eq!(materialized.materialized().poll(), Completion::Pending);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(7_u32)));
}

#[test]
fn source_run_reduce_reduces_values_via_sink_shortcut() {
  let mut materializer = RecordingMaterializer::default();
  let materialized =
    Source::from_array([1_u32, 2, 3, 4]).run_reduce(|acc, value| acc + value, &mut materializer).expect("run_reduce");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(10_u32)));
}

#[test]
fn source_run_reduce_propagates_empty_stream_failure() {
  let mut materializer = RecordingMaterializer::default();
  let materialized =
    Source::<u32, _>::empty().run_reduce(|acc, value| acc + value, &mut materializer).expect("run_reduce");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn source_run_foreach_invokes_callback_for_each_element() {
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let observed_ref = observed.clone();
  let mut materializer = RecordingMaterializer::default();
  let materialized = Source::from_array([1_u32, 2, 3])
    .run_foreach(
      move |value| {
        observed_ref.lock().push(value);
      },
      &mut materializer,
    )
    .expect("run_foreach");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(StreamDone::new())));
  assert_eq!(*observed.lock(), vec![1_u32, 2, 3]);
}

#[test]
fn source_map_materialized_value_transforms_materialized_value_and_keeps_data_path_behavior() {
  let (_graph, materialized) = Source::single(1_u32).map_materialized_value(|_| 99_u32).into_parts();
  assert_eq!(materialized, 99_u32);

  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .map_materialized_value(|_| 42_u32)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);

  let graph = Source::single(7_u32).map_materialized_value(|_| 55_u32).into_mat(Sink::ignore(), KeepLeft);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(*materialized.materialized(), 55_u32);
}

#[test]
fn materialized_unique_kill_switch_abort_fails_stream() {
  let source = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new());
  let graph = source.into_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.unique_kill_switch();

  kill_switch.abort(StreamError::Failed);
  let _ = materialized.stream().drive();

  assert_eq!(materialized.stream().state(), StreamState::Failed);
}

#[test]
fn materialized_unique_kill_switch_abort_stops_reporting_progress_after_failure() {
  let source = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new());
  let graph = source.into_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.unique_kill_switch();

  kill_switch.abort(StreamError::Failed);
  assert_eq!(materialized.stream().drive(), DriveOutcome::Progressed);
  assert_eq!(materialized.stream().state(), StreamState::Failed);
  assert_eq!(materialized.stream().drive(), DriveOutcome::Idle);
}

#[test]
fn materialized_shared_kill_switch_shutdown_completes_stream() {
  let source = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new());
  let graph = source.into_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.shared_kill_switch();

  kill_switch.shutdown();
  for _ in 0..4 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }

  assert_eq!(materialized.stream().state(), StreamState::Completed);
}

#[test]
fn materialized_unique_kill_switch_ignores_later_abort_after_shutdown() {
  let source = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new());
  let graph = source.into_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.unique_kill_switch();

  kill_switch.shutdown();
  kill_switch.abort(StreamError::Failed);

  for _ in 0..4 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }
  assert_eq!(materialized.stream().state(), StreamState::Completed);
}

#[test]
fn materialized_shared_kill_switch_shutdown_cancels_upstream_once() {
  let cancel_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let source = Source::<u32, _>::from_logic(StageKind::Custom, CancelAwareSourceLogic::new(cancel_count.clone()));
  let graph = source.into_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.shared_kill_switch();

  kill_switch.shutdown();
  for _ in 0..4 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }

  assert_eq!(materialized.stream().state(), StreamState::Completed);
  assert_eq!(*cancel_count.lock(), 1);
}

#[test]
fn materialized_unique_kill_switch_abort_cancels_upstream_once() {
  let cancel_count = ArcShared::new(SpinSyncMutex::new(0_u32));
  let source = Source::<u32, _>::from_logic(StageKind::Custom, CancelAwareSourceLogic::new(cancel_count.clone()));
  let graph = source.into_mat(Sink::ignore(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let kill_switch = materialized.unique_kill_switch();

  kill_switch.abort(StreamError::Failed);
  let _ = materialized.stream().drive();

  assert_eq!(materialized.stream().state(), StreamState::Failed);
  assert_eq!(*cancel_count.lock(), 1);
}

#[test]
fn shared_kill_switch_created_before_materialization_controls_multiple_streams() {
  let shared_kill_switch = SharedKillSwitch::new();
  let graph_left = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .into_mat(Sink::ignore(), KeepRight)
    .with_shared_kill_switch(&shared_kill_switch);
  let graph_right = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .into_mat(Sink::ignore(), KeepRight)
    .with_shared_kill_switch(&shared_kill_switch);
  let mut materializer = RecordingMaterializer::default();

  let left = graph_left.run(&mut materializer).expect("left materialize");
  let right = graph_right.run(&mut materializer).expect("right materialize");

  for _ in 0..3 {
    let _ = left.stream().drive();
    let _ = right.stream().drive();
  }
  assert_eq!(left.stream().state(), StreamState::Running);
  assert_eq!(right.stream().state(), StreamState::Running);

  shared_kill_switch.shutdown();
  for _ in 0..8 {
    let _ = left.stream().drive();
    let _ = right.stream().drive();
    if left.stream().state().is_terminal() && right.stream().state().is_terminal() {
      break;
    }
  }

  assert_eq!(left.stream().state(), StreamState::Completed);
  assert_eq!(right.stream().state(), StreamState::Completed);
}

#[test]
fn source_broadcast_with_single_fan_out_keeps_element() {
  let values =
    Source::single(5_u32).broadcast(1).expect("broadcast").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_empty_completes_without_elements() {
  let values = Source::<u32, _>::empty().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_from_option_emits_present_value() {
  let values = Source::from_option(Some(7_u32)).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_from_option_none_completes_without_elements() {
  let values = Source::<u32, _>::from_option(None).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_from_iterator_emits_values_in_order() {
  let values = Source::from_iterator([1_u32, 2, 3, 4]).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2, 3, 4]);
}

#[test]
fn source_from_iterator_empty_iterator_completes_without_elements() {
  let values =
    Source::from_iterator(core::iter::empty::<u32>()).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_from_array_emits_values_in_order() {
  let values = Source::from_array([1_u32, 2, 3, 4]).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2, 3, 4]);
}

#[test]
fn source_from_array_empty_array_completes_without_elements() {
  let values = Source::<u32, _>::from_array([]).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_from_alias_emits_values_in_order() {
  let values = Source::from([4_u32, 5, 6]).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![4_u32, 5, 6]);
}

#[test]
fn source_failed_returns_error_on_collection() {
  let values = Source::<u32, _>::failed(StreamError::Failed).run_with_collect_sink();
  assert_eq!(values, Err(StreamError::Failed));
}

#[test]
fn source_never_with_take_returns_would_block() {
  let values = Source::<u32, _>::never().take(1).run_with_collect_sink();
  assert_eq!(values, Err(StreamError::WouldBlock));
}

#[test]
fn source_range_emits_inclusive_sequence() {
  let values = Source::range(2, 5).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![2, 3, 4, 5]);
}

#[test]
fn source_range_descending_emits_reverse_sequence() {
  let values = Source::range(5, 2).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5, 4, 3, 2]);
}

#[test]
fn source_repeat_with_take_limits_elements() {
  let mut logic = RepeatSourceLogic { value: 9_u32 };
  let mut values = Vec::new();
  for _ in 0..4 {
    let value = logic.pull().expect("pull").expect("value");
    values.push(*value.downcast::<u32>().expect("u32 value"));
  }
  assert_eq!(values, vec![9_u32, 9, 9, 9]);
}

#[test]
fn source_cycle_repeats_input_sequence() {
  let mut logic = CycleSourceLogic { values: vec![1_u32, 2, 3], index: 0 };
  let mut values = Vec::new();
  for _ in 0..7 {
    let value = logic.pull().expect("pull").expect("value");
    values.push(*value.downcast::<u32>().expect("u32 value"));
  }
  assert_eq!(values, vec![1_u32, 2, 3, 1, 2, 3, 1]);
}

#[test]
fn source_cycle_empty_values_completes_without_elements() {
  let values =
    Source::<u32, _>::cycle(core::iter::empty::<u32>()).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_iterate_emits_progressive_values() {
  let mut logic = IterateSourceLogic { current: 1_u32, func: |value| value + 2 };
  let mut values = Vec::new();
  for _ in 0..4 {
    let value = logic.pull().expect("pull").expect("value");
    values.push(*value.downcast::<u32>().expect("u32 value"));
  }
  assert_eq!(values, vec![1_u32, 3, 5, 7]);
}

#[test]
fn source_as_source_with_context_attaches_unit_context() {
  let values = Source::from_array([1_u32, 2_u32])
    .into_source_with_context()
    .into_source()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![((), 1_u32), ((), 2_u32)]);
}

// --- watch_termination tests ---

#[test]
fn source_watch_termination_mat_keep_left_passes_elements_through() {
  let values = Source::from_array([5_u32, 6_u32])
    .watch_termination_mat(KeepLeft)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_watch_termination_mat_keep_right_exposes_completion_handle() {
  let source = Source::from_array([1_u32, 2_u32]).watch_termination_mat(KeepRight);
  let completion = source.map_materialized_value(|c| {
    assert_eq!(c.poll(), Completion::Pending);
    c
  });
  let values = completion.run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_watch_termination_mat_keep_both() {
  let (_graph, (left, right)) = Source::<u32, StreamNotUsed>::empty().watch_termination_mat(KeepBoth).into_parts();
  assert_eq!(left, StreamNotUsed::new());
  assert_eq!(right.poll(), Completion::Pending);
}

#[test]
fn source_combine_merges_all_sources() {
  let mut values = Source::combine([Source::from_array([1_u32, 2_u32]), Source::from_array([9_u32])])
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  values.sort();
  assert_eq!(values, vec![1_u32, 2_u32, 9_u32]);
}

#[test]
fn source_from_java_stream_alias_emits_values() {
  let values = Source::from_java_stream([3_u32, 4_u32]).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn source_from_publisher_alias_emits_values() {
  let values = Source::from_publisher([5_u32, 6_u32]).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_future_alias_emits_when_ready() {
  let values = Source::future(ready(7_u32)).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_completion_stage_alias_emits_when_ready() {
  let values = Source::completion_stage(ready(8_u32)).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn source_lazy_future_alias_emits_when_ready() {
  let values = Source::lazy_future(|| ready(9_u32)).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![9_u32]);
}

#[test]
fn source_lazy_single_alias_emits_factory_value() {
  let values = Source::lazy_single(|| 10_u32).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![10_u32]);
}

#[test]
fn source_lazy_source_emits_all_elements_from_factory() {
  let values =
    Source::lazy_source(|| Source::from_array([1_u32, 2, 3])).run_with_collect_sink().expect("run_with_collect_sink");
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
  let values = source.run_with_collect_sink().expect("run_with_collect_sink");
  // ファクトリが呼ばれ、値が取得される
  assert!(*called.lock());
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn source_lazy_source_with_empty_factory_completes_immediately() {
  let values = Source::<u32, _>::lazy_source(Source::empty).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_lazy_source_with_mapped_source_emits_transformed() {
  let values = Source::lazy_source(|| Source::from_array([1_u32, 2, 3]).map(|v| v * 10))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![10_u32, 20, 30]);
}

#[test]
fn source_lazy_source_collects_all_values_from_nested_broadcast() {
  let values = Source::lazy_source(|| Source::single(7_u32).broadcast(2).expect("broadcast"))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32, 7_u32]);
}

#[test]
fn source_lazy_source_retries_single_island_until_nested_future_is_ready() {
  let values = Source::lazy_source(|| Source::future(YieldThenOutputFuture::new(12_u32)))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![12_u32]);
}

#[test]
fn source_lazy_source_returns_would_block_for_never_ready_single_island_future() {
  let result = Source::lazy_source(|| Source::future(NeverReadyFuture::<u32>::new())).run_with_collect_sink();
  assert_eq!(result, Err(StreamError::WouldBlock));
}

#[test]
fn source_lazy_source_drains_multi_island_nested_source_until_ready() {
  let values = Source::lazy_source(|| Source::future(YieldThenOutputFuture::new(21_u32)).r#async())
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![21_u32]);
}

#[test]
fn source_lazy_source_returns_would_block_for_never_ready_multi_island_future() {
  let result = Source::lazy_source(|| Source::future(NeverReadyFuture::<u32>::new()).r#async()).run_with_collect_sink();
  assert_eq!(result, Err(StreamError::WouldBlock));
}

#[test]
fn source_maybe_alias_matches_from_option_behavior() {
  let values = Source::maybe(Some(11_u32)).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![11_u32]);
}

#[test]
fn source_queue_materializes_bounded_queue_and_emits_offered_values() {
  let source = Source::queue(4).expect("queue");
  let (graph, mut queue) = source.into_parts();
  assert_eq!(queue.offer(12_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(13_u32), QueueOfferResult::Enqueued);
  queue.complete();
  let values: Vec<u32> =
    Source::<u32, _>::from_graph(graph, queue).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![12_u32, 13_u32]);
}

#[test]
fn source_queue_take_should_not_panic_when_queue_is_already_completed() {
  let source = Source::queue(4)
    .expect("queue")
    .map_materialized_value(|mut queue| {
      assert_eq!(queue.offer(12_u32), QueueOfferResult::Enqueued);
      assert_eq!(queue.offer(13_u32), QueueOfferResult::Enqueued);
      queue.complete();
      queue
    })
    .take(1);

  let values = source.run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![12_u32]);
}

#[test]
fn source_queue_cancel_closes_queue_and_discards_buffered_values() {
  let mut queue = SourceQueue::new();
  let mut logic = UnboundedQueueSourceLogic { queue: queue.clone() };

  assert_eq!(queue.offer(12_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(13_u32), QueueOfferResult::Enqueued);

  logic.on_cancel().expect("on_cancel");

  assert!(queue.is_closed());
  assert!(queue.is_empty());
  assert!(queue.is_drained());
  assert_eq!(queue.offer(14_u32), QueueOfferResult::QueueClosed);
}

#[test]
fn source_queue_unbounded_materializes_source_queue_and_emits_offered_values() {
  let source = Source::<u32, _>::queue_unbounded();
  let (graph, mut queue) = source.into_parts();
  assert_eq!(queue.offer(20_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(21_u32), QueueOfferResult::Enqueued);
  queue.complete();

  let values: Vec<u32> =
    Source::<u32, _>::from_graph(graph, queue).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![20_u32, 21_u32]);
}

#[test]
fn source_queue_rejects_zero_capacity() {
  let result = Source::<u32, _>::queue(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_queue_with_overflow_materializes_queue_with_complete_and_emits_offered_values() {
  let source = Source::queue_with_overflow(4, OverflowStrategy::DropTail).expect("queue_with_overflow");
  let (graph, mut queue) = source.into_parts();
  assert_eq!(poll_ready(queue.offer(30_u32)), QueueOfferResult::Enqueued);
  assert_eq!(poll_ready(queue.offer(31_u32)), QueueOfferResult::Enqueued);
  let completion = queue.watch_completion();
  assert_eq!(completion.poll(), Completion::Pending);
  queue.complete();
  let values: Vec<u32> =
    Source::<u32, _>::from_graph(graph, queue).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![30_u32, 31_u32]);
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn source_bounded_queue_cancel_closes_queue_and_discards_buffered_values() {
  let mut queue = BoundedSourceQueue::new(2, OverflowStrategy::DropTail);
  let mut logic = QueueSourceLogic { queue: queue.clone() };

  assert_eq!(queue.offer(20_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(21_u32), QueueOfferResult::Enqueued);

  logic.on_cancel().expect("on_cancel");

  assert!(queue.is_closed());
  assert!(queue.is_empty());
  assert!(queue.is_drained());
  assert_eq!(queue.offer(22_u32), QueueOfferResult::QueueClosed);
}

#[test]
fn source_queue_with_overflow_allows_zero_capacity() {
  let source = Source::<u32, _>::queue_with_overflow(0, OverflowStrategy::Backpressure).expect("queue_with_overflow");
  let (_graph, mut queue) = source.into_parts();
  let mut waiting_offer = pin!(queue.offer(30_u32));
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(queue.poll().expect("poll"), Some(30_u32));
  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn source_queue_with_overflow_allows_multiple_pending_offers_when_configured() {
  let source = Source::queue_with_overflow_and_max_concurrent_offers(1, OverflowStrategy::Backpressure, 2)
    .expect("queue_with_overflow");
  let (_graph, mut queue) = source.into_parts();
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert_eq!(poll_ready(queue.offer(30_u32)), QueueOfferResult::Enqueued);

  let mut first_pending_offer = pin!(queue.offer(31_u32));
  let mut second_pending_offer = pin!(queue.offer(32_u32));

  assert_eq!(first_pending_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(second_pending_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(poll_ready(queue.offer(33_u32)), QueueOfferResult::Failure(StreamError::WouldBlock));

  assert_eq!(queue.poll().expect("poll"), Some(30_u32));
  assert_eq!(first_pending_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), Some(31_u32));
  assert_eq!(second_pending_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), Some(32_u32));
}

#[test]
fn source_queue_with_overflow_cancel_resolves_pending_offers_and_completion() {
  let mut queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 1);
  let completion = queue.watch_completion();
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);
  let mut logic = QueueWithOverflowSourceLogic { queue: queue.clone() };

  assert_eq!(poll_ready(queue.offer(30_u32)), QueueOfferResult::Enqueued);

  let mut pending_offer = pin!(queue.offer(31_u32));
  assert_eq!(pending_offer.as_mut().poll(&mut context), Poll::Pending);

  logic.on_cancel().expect("on_cancel");
  assert_eq!(pending_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::QueueClosed));
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
  assert!(queue.is_closed());
  assert!(queue.is_empty());
}

#[test]
fn source_create_defers_producer_until_source_is_materialized() {
  let called = ArcShared::new(SpinSyncMutex::new(false));
  let called_clone = called.clone();

  let source = Source::create(2, move |mut queue| {
    *called_clone.lock() = true;
    assert_eq!(queue.offer(40_u32), QueueOfferResult::Enqueued);
    assert_eq!(queue.offer(41_u32), QueueOfferResult::Enqueued);
    queue.complete();
  })
  .expect("create");

  assert!(!*called.lock());
  let graph = source.into_mat(Sink::queue(), KeepBoth);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let sink_queue = &materialized.materialized().1;
  let mut values = Vec::new();

  for _ in 0..scaled_attempts(THREAD_SYNC_ATTEMPTS) {
    let _ = materialized.stream().drive();
    while let Some(value) = sink_queue.pull() {
      values.push(value);
    }
    if materialized.stream().state().is_terminal() {
      break;
    }
    thread::yield_now();
  }

  assert!(*called.lock());
  assert_eq!(materialized.stream().state(), StreamState::Completed);
  assert_eq!(values, vec![40_u32, 41_u32]);
}

#[test]
fn source_create_take_should_not_panic_when_producer_already_completed_queue() {
  let source = Source::create(2, |mut queue| {
    assert_eq!(queue.offer(40_u32), QueueOfferResult::Enqueued);
    assert_eq!(queue.offer(41_u32), QueueOfferResult::Enqueued);
    queue.complete();
  })
  .expect("create")
  .take(1);

  let values = source.run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![40_u32]);
}

#[test]
fn source_create_auto_completes_queue_when_producer_returns_without_termination() {
  let source = Source::create(2, |mut queue| {
    assert_eq!(queue.offer(50_u32), QueueOfferResult::Enqueued);
    assert_eq!(queue.offer(51_u32), QueueOfferResult::Enqueued);
  })
  .expect("create");

  let graph = source.into_mat(Sink::queue(), KeepBoth);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let sink_queue = &materialized.materialized().1;
  let mut values = Vec::new();

  for _ in 0..scaled_attempts(THREAD_SYNC_ATTEMPTS) {
    let _ = materialized.stream().drive();
    while let Some(value) = sink_queue.pull() {
      values.push(value);
    }
    if materialized.stream().state().is_terminal() {
      break;
    }
    thread::yield_now();
  }

  assert_eq!(materialized.stream().state(), StreamState::Completed);
  assert_eq!(values, vec![50_u32, 51_u32]);
}

#[test]
fn source_create_tolerates_producer_delay_without_std_sleep() {
  let resume_second_offer = Arc::new(AtomicBool::new(false));
  let resume_second_offer_in_closure = Arc::clone(&resume_second_offer);
  let producer_paused = Arc::new(AtomicBool::new(true));
  let producer_paused_in_closure = Arc::clone(&producer_paused);

  let source = Source::create(1, move |mut queue| {
    assert_eq!(queue.offer(60_u32), QueueOfferResult::Enqueued);
    let mut resumed = false;
    for _ in 0..scaled_attempts(10_000) {
      if resume_second_offer_in_closure.load(Ordering::SeqCst) {
        resumed = true;
        break;
      }
      thread::yield_now();
    }
    assert!(resumed, "second offer gate was never opened");
    producer_paused_in_closure.store(false, Ordering::SeqCst);
    let mut second_enqueued = false;
    for _ in 0..scaled_attempts(10_000) {
      match queue.offer(61_u32) {
        | QueueOfferResult::Enqueued => {
          second_enqueued = true;
          break;
        },
        | QueueOfferResult::Failure(StreamError::WouldBlock) => thread::yield_now(),
        | other => panic!("unexpected queue result: {other:?}"),
      }
    }
    assert!(second_enqueued, "second element should be enqueued after downstream pulls");
    queue.complete();
  })
  .expect("create");

  let graph = source.into_mat(Sink::queue(), KeepBoth);
  let mut materializer = RecordingMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  let sink_queue = &materialized.materialized().1;

  let mut first_value = None;
  for _ in 0..scaled_attempts(THREAD_SYNC_ATTEMPTS) {
    let started_at = Instant::now();
    let _ = materialized.stream().drive();
    assert!(
      started_at.elapsed() < scaled_duration(Duration::from_millis(12)),
      "producer start wait must not block drive"
    );
    if let Some(value) = sink_queue.pull() {
      first_value = Some(value);
      break;
    }
    thread::yield_now();
  }
  assert_eq!(first_value, Some(60_u32));

  for _ in 0..scaled_attempts(4) {
    let started_at = Instant::now();
    let outcome = materialized.stream().drive();
    assert_eq!(outcome, DriveOutcome::Idle, "paused producer must leave the stream idle without synthetic progress");
    assert!(started_at.elapsed() < scaled_duration(Duration::from_millis(12)), "paused producer must not block drive");
    assert!(producer_paused.load(Ordering::SeqCst));
    assert_eq!(sink_queue.pull(), None);
  }

  resume_second_offer.store(true, Ordering::SeqCst);

  let mut second_value = None;
  for _ in 0..scaled_attempts(THREAD_SYNC_ATTEMPTS) {
    let _ = materialized.stream().drive();
    if let Some(value) = sink_queue.pull() {
      second_value = Some(value);
      break;
    }
    thread::yield_now();
  }
  assert_eq!(second_value, Some(61_u32));

  for _ in 0..scaled_attempts(16) {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
    thread::yield_now();
  }
  assert_eq!(materialized.stream().state(), StreamState::Completed);
}

#[test]
fn source_create_propagates_queue_failure_from_producer() {
  let source = Source::<u32, _>::create(2, |mut queue| {
    queue.fail(StreamError::Failed);
  })
  .expect("create");

  let result = source.run_with_collect_sink();
  assert_eq!(result, Err(StreamError::Failed));
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
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
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
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
  assert_eq!(values, vec![0_u32, 1_u32, 2_u32]);
}

#[test]
fn source_zip_n_alias_wraps_values_by_fan_in() {
  let values = Source::single(15_u32).zip_n(1).expect("zip_n").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![vec![15_u32]]);
}

#[test]
fn source_zip_with_n_alias_maps_zipped_values() {
  let values = Source::single(16_u32)
    .zip_with_n(1, |items: Vec<u32>| items.into_iter().sum::<u32>())
    .expect("zip_with_n")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![16_u32]);
}

#[test]
fn source_from_input_stream_alias_emits_values() {
  let values = Source::from_input_stream([17_u32, 18_u32]).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![17_u32, 18_u32]);
}

#[test]
fn source_from_output_stream_alias_emits_values() {
  let values = Source::from_output_stream([19_u32, 20_u32]).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![19_u32, 20_u32]);
}

#[test]
fn source_from_iterator_emits_bytes() {
  // from_path は deprecated のため、同等の from_iterator を使用
  let values = Source::from_iterator("ab".as_bytes().to_vec()).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![b'a', b'b']);
}

#[test]
fn source_broadcast_rejects_zero_fan_out() {
  assert!(Source::single(1_u32).broadcast(0).is_err());
}

#[test]
fn source_balance_keeps_single_path_behavior() {
  let values =
    Source::single(5_u32).balance(1).expect("balance").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_balance_rejects_zero_fan_out() {
  assert!(Source::single(1_u32).balance(0).is_err());
}

#[test]
fn source_merge_keeps_single_path_behavior() {
  let values = Source::single(5_u32).merge(1).expect("merge").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_merge_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).merge(0).is_err());
}

#[test]
fn source_zip_wraps_value_when_single_path() {
  let values = Source::single(5_u32).zip(1).expect("zip").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_zip_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).zip(0).is_err());
}

#[test]
fn source_concat_keeps_single_path_behavior() {
  let values = Source::single(5_u32).concat(1).expect("concat").run_with_collect_sink().expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

#[test]
fn source_unzip_emits_tuple_components() {
  let values = Source::single((5_u32, 6_u32)).unzip().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_unzip_with_emits_mapped_tuple_components() {
  let values = Source::single(5_u32)
    .unzip_with(|value| (value, value.saturating_add(1)))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_interleave_keeps_single_path_behavior() {
  let values =
    Source::single(5_u32).interleave(1).expect("interleave").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_interleave_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).interleave(0).is_err());
}

#[test]
fn source_prepend_keeps_single_path_behavior() {
  let values =
    Source::single(5_u32).prepend(1).expect("prepend").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_prepend_rejects_zero_fan_in() {
  assert!(Source::single(1_u32).prepend(0).is_err());
}

#[test]
fn source_zip_all_wraps_value_when_single_path() {
  let values =
    Source::single(5_u32).zip_all(1, 0_u32).expect("zip_all").run_with_collect_sink().expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_flat_map_merge_preserves_outer_order_and_round_robin() {
  let values = Source::from_array([1_u32, 2_u32])
    .flat_map_merge(2, |value| Source::from_array([value, value.saturating_add(10)]))
    .expect("flat_map_merge")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![12_u32, 13_u32]);
}

#[test]
fn source_flat_map_merge_emits_head_without_waiting_for_inner_completion() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let inner_pulls = pulls.clone();
  let mut materializer = RecordingMaterializer::default();
  let materialized = Source::single(0_u32)
    .flat_map_merge(1, move |_| {
      Source::<u32, _>::from_logic(
        StageKind::Custom,
        CountingSequenceSourceLogic::new(&[42, 43, 44], inner_pulls.clone()),
      )
    })
    .expect("flat_map_merge")
    .run_with(Sink::head(), &mut materializer)
    .expect("run_with");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(42_u32)));
  assert_eq!(*pulls.lock(), 1_usize);
}

#[test]
fn source_flat_map_concat_keeps_order_with_empty_inner_stream() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32]).flat_map_concat(|value| {
    if value == 1 { Source::empty() } else { Source::from_array([value.saturating_add(20), value.saturating_add(30)]) }
  });
  let values = values.run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![22_u32, 32_u32, 23_u32, 33_u32]);
}

#[test]
fn source_flat_map_concat_emits_head_without_waiting_for_inner_completion() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let inner_pulls = pulls.clone();
  let mut materializer = RecordingMaterializer::default();
  let materialized = Source::single(0_u32)
    .flat_map_concat(move |_| {
      Source::<u32, _>::from_logic(
        StageKind::Custom,
        CountingSequenceSourceLogic::new(&[42, 43, 44], inner_pulls.clone()),
      )
    })
    .run_with(Sink::head(), &mut materializer)
    .expect("run_with");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(42_u32)));
  assert_eq!(*pulls.lock(), 1_usize);
}

#[test]
fn source_buffer_keeps_single_path_behavior() {
  let values = Source::single(5_u32)
    .buffer(2, OverflowStrategy::Backpressure)
    .expect("buffer")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_buffer_rejects_zero_capacity() {
  let result = Source::single(1_u32).buffer(0, OverflowStrategy::Backpressure);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_buffer_drop_new_keeps_single_path_behavior() {
  // Pekko parity: Source.buffer(_, OverflowStrategy.dropNew) must construct without
  // error and forward elements that fit within capacity. Buffer overflow semantics
  // (rejecting newly arrived elements) are exercised at queue layer; here we only
  // ensure the new variant is accepted by the buffer stage's strategy match arms.
  let values = Source::single(5_u32)
    .buffer(2, OverflowStrategy::DropNew)
    .expect("buffer")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_async_keeps_single_path_behavior() {
  let values = Source::single(5_u32).r#async().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_throttle_keeps_single_path_behavior() {
  let values = Source::single(5_u32)
    .throttle(2, ThrottleMode::Shaping)
    .expect("throttle")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_throttle_rejects_zero_capacity() {
  let result = Source::single(1_u32).throttle(0, ThrottleMode::Shaping);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_delay_keeps_single_path_behavior() {
  let values = Source::single(5_u32).delay(2).expect("delay").run_with_collect_sink().expect("run_with_collect_sink");
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
  let values = Source::single(5_u32)
    .initial_delay(2)
    .expect("initial_delay")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![2_u32, 4_u32]);
}

#[test]
fn source_filter_not_keeps_non_matching_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .filter_not(|value| value % 2 == 0)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 3_u32]);
}

#[test]
fn source_flatten_optional_emits_present_value() {
  let values = Source::single(Some(7_u32)).flatten_optional().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_flatten_optional_skips_none() {
  let values = Source::single(None::<u32>).flatten_optional().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn source_collect_maps_present_values_and_skips_absent_values() {
  let values = Source::from_array([1_i32, -1_i32, 2_i32])
    .collect(|value| u32::try_from(value).ok())
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_flatten_flattens_nested_sources_in_order_and_skips_empty_inner_sources() {
  let values =
    Source::from_array([Source::empty(), Source::from_array([22_u32, 32_u32]), Source::from_array([23_u32, 33_u32])])
      .flatten()
      .run_with_collect_sink()
      .expect("run_with_collect_sink");

  assert_eq!(values, vec![22_u32, 32_u32, 23_u32, 33_u32]);
}

#[test]
fn source_flatten_emits_inner_head_without_waiting_for_inner_completion() {
  let (inner_graph, mut inner_queue) = Source::<u32, _>::queue_unbounded().into_parts();
  let inner = Source::from_graph(inner_graph, StreamNotUsed::new());
  assert_eq!(inner_queue.offer(42_u32), QueueOfferResult::Enqueued);
  let mut materializer = RecordingMaterializer::default();

  let materialized = Source::single(inner).flatten().run_with(Sink::head(), &mut materializer).expect("run_with");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(42_u32)));
}

#[test]
fn source_map_async_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .map_async(2, |value| async move { value.saturating_add(1) })
    .expect("map_async")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 11_u32, 2_u32, 12_u32, 3_u32, 13_u32]);
}

#[test]
fn source_map_option_emits_only_present_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .map_option(|value| if value % 2 == 0 { Some(value) } else { None })
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 101_u32, 3_u32, 103_u32, 6_u32, 106_u32]);
}

#[test]
fn source_stateful_map_on_complete_emits_final_element() {
  // 準備: on_complete で蓄積した合計値を末尾に出力する stateful_map
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .stateful_map_with_on_complete(
      || 0_u32,
      |state, value| {
        *state = state.saturating_add(value);
        value
      },
      |state| Some(state),
    )
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 通常要素に加え、on_complete が出力した合計値が末尾に追加される
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 6_u32]);
}

#[test]
fn source_stateful_map_on_complete_none_emits_nothing_extra() {
  // 準備: on_complete が None を返す（末尾要素なし）
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .stateful_map_with_on_complete(
      || 0_u32,
      |state, value| {
        *state = state.saturating_add(value);
        value
      },
      |_state| None,
    )
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: on_complete が None を返したため、通常要素のみ
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_stateful_map_concat_with_accumulator_processes_elements() {
  // 準備: StatefulMapConcatAccumulator を使用した stateful_map_concat
  use crate::core::dsl::StatefulMapConcatAccumulator;

  struct DoublingAccumulator;

  impl StatefulMapConcatAccumulator<u32, u32> for DoublingAccumulator {
    fn apply(&mut self, input: u32) -> Vec<u32> {
      vec![input, input * 2]
    }
  }

  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .stateful_map_concat_with_accumulator(|| DoublingAccumulator)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 各要素が [value, value*2] に展開される
  assert_eq!(values, vec![1_u32, 2, 2, 4, 3, 6]);
}

#[test]
fn source_stateful_map_concat_with_accumulator_on_complete_emits_trailing() {
  // 準備: on_complete で残りのバッファを排出する accumulator
  use crate::core::dsl::StatefulMapConcatAccumulator;

  struct BufferingAccumulator {
    buffer: Vec<u32>,
  }

  impl StatefulMapConcatAccumulator<u32, u32> for BufferingAccumulator {
    fn apply(&mut self, input: u32) -> Vec<u32> {
      self.buffer.push(input);
      if self.buffer.len() >= 2 { core::mem::take(&mut self.buffer) } else { vec![] }
    }

    fn on_complete(&mut self) -> Vec<u32> {
      core::mem::take(&mut self.buffer)
    }
  }

  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .stateful_map_concat_with_accumulator(|| BufferingAccumulator { buffer: Vec::new() })
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: [1,2] はバッファ満了で排出、[3] は on_complete で排出
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn source_drop_skips_first_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .drop(2)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn source_take_limits_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .take(2)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_drop_while_skips_matching_prefix() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .drop_while(|value| *value < 3)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn source_take_while_keeps_matching_prefix() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .take_while(|value| *value < 3)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_take_until_includes_first_matching_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .take_until(|value| *value >= 3)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_grouped_emits_fixed_size_chunks() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]))
    .grouped(2)
    .expect("grouped")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![0_u32, 1_u32, 3_u32, 6_u32]);
}

#[test]
fn source_intersperse_injects_markers() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .intersperse(10_u32, 99_u32, 11_u32)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![10_u32, 1_u32, 99_u32, 2_u32, 99_u32, 3_u32, 11_u32]);
}

#[test]
fn source_intersperse_on_empty_stream_emits_start_and_end() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .intersperse(10_u32, 99_u32, 11_u32)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![10_u32, 11_u32]);
}

#[test]
fn source_zip_with_index_pairs_each_element_with_index() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[7, 8, 9]))
    .zip_with_index()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![(7_u32, 0_u64), (8_u32, 1_u64), (9_u32, 2_u64)]);
}

#[test]
fn source_group_by_keeps_single_path_behavior() {
  let values = Source::single(5_u32)
    .group_by(4, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .merge_substreams()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_group_by_rejects_zero_max_substreams() {
  let result = Source::single(1_u32).group_by(0, |value: &u32| *value, SubstreamCancelStrategy::default());
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "max_substreams", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_split_when_emits_single_segment_for_single_element() {
  let values =
    Source::single(5_u32).split_when(|_| false).into_source().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_split_when_with_cancel_strategy_emits_single_segment_for_single_element() {
  let values = Source::single(5_u32)
    .split_when_with_cancel_strategy(SubstreamCancelStrategy::Drain, |_| false)
    .into_source()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_split_after_emits_single_segment_for_single_element() {
  let values =
    Source::single(5_u32).split_after(|_| false).into_source().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_split_after_with_cancel_strategy_emits_single_segment_for_single_element() {
  let values = Source::single(5_u32)
    .split_after_with_cancel_strategy(SubstreamCancelStrategy::Propagate, |_| false)
    .into_source()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
fn source_split_when_starts_new_segment_with_matching_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .split_when(|value| value % 2 == 0)
    .into_source()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![vec![1_u32], vec![2_u32, 3_u32], vec![4_u32]]);
}

#[test]
fn source_split_after_keeps_matching_element_in_current_segment() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .split_after(|value| value % 2 == 0)
    .into_source()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32]]);
}

#[test]
fn source_merge_substreams_flattens_single_segment() {
  let values = Source::single(5_u32)
    .split_after(|_| true)
    .merge_substreams()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_concat_substreams_flattens_single_segment() {
  let values = Source::single(5_u32)
    .split_after(|_| true)
    .concat_substreams()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_merge_substreams_with_parallelism_flattens_single_segment() {
  let values = Source::single(5_u32)
    .split_after(|_| true)
    .merge_substreams_with_parallelism(2)
    .expect("merge_substreams_with_parallelism")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .group_by(2, |value: &u32| *value, SubstreamCancelStrategy::default())
    .expect("group_by")
    .merge_substreams()
    .run_with_collect_sink();
  assert_eq!(result, Err(StreamError::TooManySubstreamsOpen { max_substreams: 2 }));
}

#[test]
fn source_group_by_cancels_upstream_after_head_completion_by_default() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let mut materializer = RecordingMaterializer::default();
  let materialized =
    Source::<u32, _>::from_logic(StageKind::Custom, CountingSequenceSourceLogic::new(&[1, 2, 3], pulls.clone()))
      .group_by(4, |value: &u32| value % 2, SubstreamCancelStrategy::default())
      .expect("group_by")
      .merge_substreams()
      .run_with(Sink::head(), &mut materializer)
      .expect("run_with");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(1_u32)));
  assert_eq!(*pulls.lock(), 1_usize);
}

#[test]
fn source_p2_regression_group_by_merge_substreams_with_delay_and_zip_all() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .group_by(4, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .merge_substreams()
    .delay(1)
    .expect("delay")
    .zip_all(1, 0_u32)
    .expect("zip_all")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![4_u32, 5_u32]);
}

#[test]
fn source_map_error_transforms_upstream_failure() {
  let result =
    Source::<u32, _>::failed(StreamError::Failed).map_error(|_| StreamError::WouldBlock).run_with_collect_sink();
  assert_eq!(result, Err(StreamError::WouldBlock));
}

#[test]
fn source_on_error_continue_resumes_after_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .on_error_continue()
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_on_error_resume_alias_resumes_after_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .on_error_resume()
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_on_error_continue_if_resumes_after_matching_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .on_error_continue_if(|error| matches!(error, StreamError::Failed))
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_on_error_continue_if_with_invokes_consumer_for_matching_failure() {
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let captured = observed.clone();
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .on_error_continue_if_with(
    |error| matches!(error, StreamError::Failed),
    move |error| {
      captured.lock().push(error.clone());
    },
  )
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32]);
  assert_eq!(observed.lock().as_slice(), &[StreamError::Failed]);
}

#[test]
fn source_on_error_complete_stops_after_matching_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .on_error_complete()
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn source_on_error_complete_if_stops_on_matching_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .on_error_complete_if(|error| matches!(error, StreamError::Failed))
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn source_recover_replaces_upstream_failure_with_fallback() {
  let values = Source::<u32, _>::failed(StreamError::Failed)
    .recover(|_| Some(5_u32))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_recover_drops_later_elements_after_upstream_failure() {
  let values = Source::<u32, _>::from_logic(
    StageKind::Custom,
    FailureSequenceSourceLogic::new(&[Ok(1_u32), Err(StreamError::Failed), Ok(2_u32)]),
  )
  .recover(|_| Some(5_u32))
  .run_with_collect_sink()
  .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 5_u32]);
}

#[test]
fn source_recover_with_alias_switches_to_recovery_source() {
  let values = Source::<u32, _>::failed(StreamError::Failed)
    .recover_with(|_| Some(Source::from_array([8_u32, 9_u32])))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![8_u32, 9_u32]);
}

#[test]
fn source_recover_with_retries_fails_when_retry_budget_is_exhausted() {
  let result = Source::<u32, _>::failed(StreamError::Failed)
    .recover_with_retries(0, |_| Some(Source::single(5_u32)))
    .run_with_collect_sink();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn source_recover_with_retries_switches_recovery_sources_incrementally() {
  let mut attempts = 0_u8;
  let values = Source::<u32, _>::failed(StreamError::Failed)
    .recover_with_retries(2, move |_| {
      attempts = attempts.saturating_add(1);
      if attempts == 1 {
        Some(Source::<u32, _>::from_logic(
          StageKind::Custom,
          FailureSequenceSourceLogic::new(&[Ok(7_u32), Err(StreamError::Failed)]),
        ))
      } else {
        Some(Source::from_array([8_u32, 9_u32]))
      }
    })
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32, 8_u32, 9_u32]);
}

#[test]
fn source_recover_with_retries_fails_after_consuming_retry_budget() {
  let result = Source::<u32, _>::failed(StreamError::Failed)
    .recover_with_retries(1, |_| {
      Some(Source::<u32, _>::from_logic(
        StageKind::Custom,
        FailureSequenceSourceLogic::new(&[Ok(7_u32), Err(StreamError::Failed)]),
      ))
    })
    .run_with_collect_sink();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn source_restart_with_backoff_keeps_single_path_behavior() {
  let values =
    Source::single(5_u32).restart_source_with_backoff(1, 3).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_on_failures_with_backoff_alias_keeps_single_path_behavior() {
  let values =
    Source::single(5_u32).on_failures_with_backoff(1, 3).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_with_backoff_alias_keeps_single_path_behavior() {
  let values = Source::single(5_u32).with_backoff(1, 3).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_with_backoff_and_context_alias_keeps_single_path_behavior() {
  let values = Source::single(5_u32)
    .with_backoff_and_context(1, 3, "compat")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_restart_with_settings_keeps_single_path_behavior() {
  let settings = RestartConfig::new(1, 4, 3)
    .with_random_factor_permille(250)
    .with_max_restarts_within_ticks(16)
    .with_jitter_seed(11);
  let values = Source::single(5_u32)
    .restart_source_with_settings(settings)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_supervision_variants_keep_single_path_behavior() {
  let values = Source::single(5_u32)
    .supervision_stop()
    .supervision_resume()
    .supervision_restart()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_async_preserves_elements_and_order() {
  // detach は deprecated のため、同等の r#async() を使用
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .r#async()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_fold_emits_running_accumulation_without_initial() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .fold(0_u32, |acc, value| acc + value)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn source_reduce_folds_with_first_element_as_seed() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .reduce(|acc, value| acc + value)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn source_lazy_source_persists_nested_source_failure() {
  let mut logic = LazySourceLogic::<u32, _> {
    factory: Some(|| Source::<u32, StreamNotUsed>::failed(StreamError::Failed)),
    buffer:  VecDeque::new(),
    error:   None,
    _pd:     PhantomData,
  };

  // Given: 初回 pull で factory が消費され nested source の評価が失敗する
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
fn drain_source_for_lazy_source_rejects_graph_without_tail_outlet() {
  let source = Source::<u32, StreamNotUsed>::from_graph(StreamGraph::new(), StreamNotUsed::new());
  let result = super::drain_source_for_lazy_source(source);
  assert_eq!(result, Err(StreamError::InvalidConnection));
}

#[test]
fn source_distinct_removes_duplicate_elements() {
  let values =
    Source::from_array([3_u32, 1, 2, 1, 3, 2, 4]).distinct().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![3_u32, 1, 2, 4]);
}

#[test]
fn source_distinct_on_already_unique_passes_all() {
  let values = Source::from_array([1_u32, 2, 3]).distinct().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn source_distinct_by_removes_elements_with_duplicate_key() {
  let values = Source::from_array([(1_u32, "a"), (2, "b"), (1, "c"), (3, "d")])
    .distinct_by(|pair: &(u32, &str)| pair.0)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![(1_u32, "a"), (2, "b"), (3, "d")]);
}

#[test]
fn source_from_graph_creates_source_from_existing_graph() {
  let original = Source::from_array([10_u32, 20, 30]);
  let (graph, mat) = original.into_parts();
  let reconstructed = Source::<u32, StreamNotUsed>::from_graph(graph, mat);
  let values = reconstructed.run_with_collect_sink().expect("run_with_collect_sink");
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
    .throttle(2, ThrottleMode::Enforcing)
    .expect("throttle")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_throttle_enforcing_mode_fails_on_capacity_overflow() {
  let result = Source::single(alloc::vec![1_u32, 2, 3])
    .map_concat(|v: Vec<u32>| v)
    .throttle(1, ThrottleMode::Enforcing)
    .expect("throttle")
    .run_with_collect_sink();
  assert_eq!(result, Err(StreamError::BufferOverflow));
}

#[test]
fn source_named_keeps_elements_and_sets_attributes() {
  let values =
    Source::from_array([1_u32, 2, 3]).named("test-source").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2, 3]);

  let (graph, _mat) = Source::<u32, StreamNotUsed>::from_array([1_u32, 2]).named("test-source").into_parts();
  assert_eq!(graph.attributes().names(), &[String::from("test-source")]);
}

#[test]
fn source_with_and_add_attributes_merge_names() {
  let (graph, _mat) = Source::<u32, StreamNotUsed>::from_array([1_u32, 2])
    .with_attributes(Attributes::named("base"))
    .add_attributes(Attributes::named("extra"))
    .into_parts();
  assert_eq!(graph.attributes().names(), &[String::from("base"), String::from("extra")]);
}

#[test]
fn source_from_materializer_creates_source() {
  let values = Source::from_materializer(|| Source::from_array([10_u32, 20]))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
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
  let values =
    Source::single(7_u32).debounce(1).expect("debounce").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_sample_keeps_single_path_behavior() {
  let values = Source::single(7_u32).sample(1).expect("sample").run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn combine_empty_returns_empty_source() {
  let sources: Vec<Source<u32, StreamNotUsed>> = Vec::new();
  let combined = Source::combine(sources);
  let values = combined.run_with_collect_sink().expect("run_with_collect_sink");
  assert!(values.is_empty());
}

#[test]
fn combine_single_source_returns_identity() {
  let sources = vec![Source::from_iterator(vec![1_u32, 2, 3])];
  let combined = Source::combine(sources);
  let values = combined.run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn combine_two_sources_merges_all_elements() {
  let s1 = Source::from_iterator(vec![1_u32, 2, 3]);
  let s2 = Source::from_iterator(vec![4_u32, 5, 6]);
  let combined = Source::combine(vec![s1, s2]);
  let mut values = combined.run_with_collect_sink().expect("run_with_collect_sink");
  values.sort();
  assert_eq!(values, vec![1_u32, 2, 3, 4, 5, 6]);
}

#[test]
fn combine_three_sources_merges_all_elements() {
  let s1 = Source::from_iterator(vec![1_u32]);
  let s2 = Source::from_iterator(vec![2_u32]);
  let s3 = Source::from_iterator(vec![3_u32]);
  let combined = Source::combine(vec![s1, s2, s3]);
  let mut values = combined.run_with_collect_sink().expect("run_with_collect_sink");
  values.sort();
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn combine_mat_merges_two_sources_with_keep_both() {
  let s1: Source<u32, u32> = Source::from_iterator(vec![10_u32, 20]).map_materialized_value(|_| 1_u32);
  let s2: Source<u32, u32> = Source::from_iterator(vec![30_u32, 40]).map_materialized_value(|_| 2_u32);
  let combined: Source<u32, (u32, u32)> = Source::combine_mat(s1, s2, KeepBoth);
  let mut values = combined.run_with_collect_sink().expect("run_with_collect_sink");
  values.sort();
  assert_eq!(values, vec![10_u32, 20, 30, 40]);
}

#[test]
fn combine_mat_merges_two_sources_with_keep_left() {
  let s1: Source<u32, u32> = Source::from_iterator(vec![1_u32]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_iterator(vec![2_u32]).map_materialized_value(|_| 20_u32);
  let combined: Source<u32, u32> = Source::combine_mat(s1, s2, KeepLeft);
  let mut values = combined.run_with_collect_sink().expect("run_with_collect_sink");
  values.sort();
  assert_eq!(values, vec![1_u32, 2]);
}

#[test]
fn merge_prioritized_n_empty_returns_empty_source() {
  let sources: Vec<Source<u32, StreamNotUsed>> = Vec::new();
  let values = Source::merge_prioritized_n(sources, &[])
    .expect("merge_prioritized_n")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert!(values.is_empty());
}

#[test]
fn merge_prioritized_n_single_source_returns_identity() {
  let sources = vec![Source::from_iterator(vec![1_u32, 2, 3])];
  let values = Source::merge_prioritized_n(sources, &[1])
    .expect("merge_prioritized_n")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn merge_prioritized_n_uses_weighted_merge_flow_stage() {
  let s1 = Source::from_iterator(vec![1_u32, 2, 3, 4, 5, 6]);
  let s2 = Source::from_iterator(vec![100_u32, 200, 300, 400]);
  let (graph, _) = Source::merge_prioritized_n(vec![s1, s2], &[3, 1]).expect("merge_prioritized_n").into_parts();
  let stages = graph.into_stages();
  assert!(
    stages
      .iter()
      .any(|s| matches!(s, StageDefinition::Flow(definition) if definition.kind == StageKind::FlowMergePrioritized),)
  );
}

#[test]
fn merge_prioritized_n_respects_weighted_round_robin_order() {
  let s1 = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5, 6]));
  let s2 = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[100, 200, 300, 400]));

  let values = Source::merge_prioritized_n(vec![s1, s2], &[3, 1])
    .expect("merge_prioritized_n")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  assert_eq!(values, vec![1_u32, 2, 3, 100, 4, 5, 6, 200, 300, 400]);
}

#[test]
fn merge_prioritized_n_emits_head_without_draining_never_source() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let primary =
    Source::<u32, _>::from_logic(StageKind::Custom, CountingSequenceSourceLogic::new(&[1, 2, 3], pulls.clone()));
  let secondary = Source::<u32, StreamNotUsed>::never();
  let merged = Source::merge_prioritized_n(vec![primary, secondary], &[3, 1]).expect("merge_prioritized_n");
  let mut materializer = RecordingMaterializer::default();

  let materialized = merged.run_with(Sink::head(), &mut materializer).expect("run_with");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(1_u32)));
  assert_eq!(*pulls.lock(), 1_usize);
}

#[test]
fn merge_prioritized_n_head_does_not_drain_lower_priority_source() {
  let primary_pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let secondary_pulls = ArcShared::new(SpinSyncMutex::new(0_usize));
  let primary = Source::<u32, _>::from_logic(
    StageKind::Custom,
    CountingSequenceSourceLogic::new(&[1, 2, 3], primary_pulls.clone()),
  );
  let secondary = Source::<u32, _>::from_logic(
    StageKind::Custom,
    CountingSequenceSourceLogic::new(&[100, 200, 300], secondary_pulls.clone()),
  );
  let merged = Source::merge_prioritized_n(vec![primary, secondary], &[3, 1]).expect("merge_prioritized_n");
  let mut materializer = RecordingMaterializer::default();

  let materialized = merged.run_with(Sink::head(), &mut materializer).expect("run_with");

  assert_eq!(materializer.calls, 1);
  assert_eq!(drive_materialized_completion(&materialized), Completion::Ready(Ok(1_u32)));
  assert_eq!(*primary_pulls.lock(), 1_usize);
  assert!(*secondary_pulls.lock() <= 1_usize);
}

#[test]
fn merge_prioritized_n_rejects_zero_priority() {
  let s1 = Source::single(1_u32);
  let s2 = Source::single(2_u32);
  let result = Source::merge_prioritized_n(vec![s1, s2], &[3, 0]);
  assert!(result.is_err());
}

#[test]
fn merge_prioritized_n_rejects_length_mismatch() {
  let s1 = Source::single(1_u32);
  let s2 = Source::single(2_u32);
  let result = Source::merge_prioritized_n(vec![s1, s2], &[3]);
  assert!(result.is_err());
}

// --- A4: Source.group_by with SubstreamCancelStrategy ---

#[test]
fn source_group_by_with_propagate_strategy_creates_subflow() {
  // Given: a source with group_by using Propagate strategy
  use crate::core::SubstreamCancelStrategy;

  let source = Source::from_array([1_u32, 2, 3, 4, 5, 6]);

  // When: calling group_by with SubstreamCancelStrategy
  let result = source.group_by(10, |x| x % 2, SubstreamCancelStrategy::Propagate);

  // Then: the subflow is created successfully
  assert!(result.is_ok());
}

#[test]
fn source_group_by_with_drain_strategy_creates_subflow() {
  // Given: a source with group_by using Drain strategy
  use crate::core::SubstreamCancelStrategy;

  let source = Source::from_array([1_u32, 2, 3, 4, 5, 6]);

  // When: calling group_by with Drain strategy
  let result = source.group_by(10, |x| x % 2, SubstreamCancelStrategy::Drain);

  // Then: the subflow is created successfully
  assert!(result.is_ok());
}

// ===========================================================================
// *Mat バリアント（Source 版）
// ===========================================================================

// ---------------------------------------------------------------------------
// zip_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_zip_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::single(1_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(2_u32).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.zip_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_zip_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::single(1_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(2_u32).map_materialized_value(|_| 20_u32);

  let values = s1.zip_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![vec![1_u32, 2_u32]]);
}

// ---------------------------------------------------------------------------
// zip_all_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_zip_all_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::from_array([1_u32, 2]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([3_u32]).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.zip_all_mat(s2, 0_u32, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_zip_all_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::from_array([1_u32, 2]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([3_u32]).map_materialized_value(|_| 20_u32);

  let values = s1.zip_all_mat(s2, 0_u32, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![vec![1_u32, 3_u32], vec![2_u32, 0_u32]]);
}

// ---------------------------------------------------------------------------
// zip_with_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_zip_with_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::single(10_u32).map_materialized_value(|_| 1_u32);
  let s2: Source<u32, u32> = Source::single(20_u32).map_materialized_value(|_| 2_u32);

  let (_graph, (left_mat, right_mat)) =
    s1.zip_with_mat(s2, |values: Vec<u32>| values.into_iter().sum::<u32>(), KeepBoth).into_parts();

  assert_eq!(left_mat, 1_u32);
  assert_eq!(right_mat, 2_u32);
}

#[test]
fn source_zip_with_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::single(10_u32).map_materialized_value(|_| 1_u32);
  let s2: Source<u32, u32> = Source::single(20_u32).map_materialized_value(|_| 2_u32);

  let values = s1
    .zip_with_mat(s2, |values: Vec<u32>| values.into_iter().sum::<u32>(), KeepLeft)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  assert_eq!(values, vec![30_u32]);
}

// ---------------------------------------------------------------------------
// zip_latest_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_zip_latest_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::single(1_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(2_u32).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.zip_latest_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_zip_latest_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::single(1_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(2_u32).map_materialized_value(|_| 20_u32);

  let values = s1.zip_latest_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![vec![1_u32, 2_u32]]);
}

// ---------------------------------------------------------------------------
// zip_latest_with_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_zip_latest_with_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::single(10_u32).map_materialized_value(|_| 1_u32);
  let s2: Source<u32, u32> = Source::single(20_u32).map_materialized_value(|_| 2_u32);

  let (_graph, (left_mat, right_mat)) =
    s1.zip_latest_with_mat(s2, |values: Vec<u32>| values.into_iter().sum::<u32>(), KeepBoth).into_parts();

  assert_eq!(left_mat, 1_u32);
  assert_eq!(right_mat, 2_u32);
}

#[test]
fn source_zip_latest_with_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::single(10_u32).map_materialized_value(|_| 1_u32);
  let s2: Source<u32, u32> = Source::single(20_u32).map_materialized_value(|_| 2_u32);

  let values = s1
    .zip_latest_with_mat(s2, |values: Vec<u32>| values.into_iter().sum::<u32>(), KeepLeft)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  assert_eq!(values, vec![30_u32]);
}

// ---------------------------------------------------------------------------
// merge_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_merge_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::single(7_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(8_u32).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.merge_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_merge_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::single(7_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(8_u32).map_materialized_value(|_| 20_u32);

  let mut values = s1.merge_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");
  values.sort();

  assert_eq!(values, vec![7_u32, 8_u32]);
}

// ---------------------------------------------------------------------------
// merge_latest_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_merge_latest_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::single(7_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(8_u32).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.merge_latest_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_merge_latest_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::single(7_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(8_u32).map_materialized_value(|_| 20_u32);

  let values = s1.merge_latest_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");

  // merge_latest emits Vec of latest values from all inputs
  assert!(!values.is_empty());
}

// ---------------------------------------------------------------------------
// merge_preferred_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_merge_preferred_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::single(7_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(8_u32).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.merge_preferred_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_merge_preferred_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::single(7_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(8_u32).map_materialized_value(|_| 20_u32);

  let mut values = s1.merge_preferred_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");
  values.sort();

  assert_eq!(values, vec![7_u32, 8_u32]);
}

// ---------------------------------------------------------------------------
// merge_prioritized_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_merge_prioritized_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::single(7_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(8_u32).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.merge_prioritized_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_merge_prioritized_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::single(7_u32).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::single(8_u32).map_materialized_value(|_| 20_u32);

  let mut values = s1.merge_prioritized_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");
  values.sort();

  assert_eq!(values, vec![7_u32, 8_u32]);
}

// ---------------------------------------------------------------------------
// merge_sorted_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_merge_sorted_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::from_array([1_u32, 3, 5]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([2_u32, 4, 6]).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.merge_sorted_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_merge_sorted_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::from_array([1_u32, 3, 5]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([2_u32, 4, 6]).map_materialized_value(|_| 20_u32);

  let values = s1.merge_sorted_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![1_u32, 2, 3, 4, 5, 6]);
}

// ---------------------------------------------------------------------------
// concat_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_concat_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::from_array([1_u32, 2]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([3_u32, 4]).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.concat_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_concat_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::from_array([1_u32, 2]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([3_u32, 4]).map_materialized_value(|_| 20_u32);

  let values = s1.concat_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![1_u32, 2, 3, 4]);
}

// ---------------------------------------------------------------------------
// prepend_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_prepend_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::from_array([3_u32, 4]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([1_u32, 2]).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.prepend_mat(s2, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_prepend_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::from_array([3_u32, 4]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([1_u32, 2]).map_materialized_value(|_| 20_u32);

  let values = s1.prepend_mat(s2, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![1_u32, 2, 3, 4]);
}

// ---------------------------------------------------------------------------
// interleave_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_interleave_mat_combines_materialized_values() {
  let s1: Source<u32, u32> = Source::from_array([1_u32, 3]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([2_u32, 4]).map_materialized_value(|_| 20_u32);

  let (_graph, (left_mat, right_mat)) = s1.interleave_mat(s2, 1, KeepBoth).into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_interleave_mat_preserves_data_path_behavior() {
  let s1: Source<u32, u32> = Source::from_array([1_u32, 3]).map_materialized_value(|_| 10_u32);
  let s2: Source<u32, u32> = Source::from_array([2_u32, 4]).map_materialized_value(|_| 20_u32);

  let mut values = s1.interleave_mat(s2, 1, KeepLeft).run_with_collect_sink().expect("run_with_collect_sink");
  values.sort();

  assert_eq!(values, vec![1_u32, 2, 3, 4]);
}

// ---------------------------------------------------------------------------
// flat_map_prefix_mat (Source)
// ---------------------------------------------------------------------------

#[test]
fn source_flat_map_prefix_mat_combines_materialized_values() {
  use crate::core::dsl::Flow;

  let source: Source<u32, u32> = Source::from_array([1_u32, 2, 3]).map_materialized_value(|_| 10_u32);

  let (_graph, (left_mat, right_mat)) = source
    .flat_map_prefix_mat(
      1,
      |_prefix: Vec<u32>| Flow::<u32, u32, StreamNotUsed>::new().map_materialized_value(|_| 20_u32),
      KeepBoth,
    )
    .into_parts();

  assert_eq!(left_mat, 10_u32);
  assert_eq!(right_mat, 20_u32);
}

#[test]
fn source_flat_map_prefix_mat_preserves_data_path_behavior() {
  use crate::core::dsl::Flow;

  let source: Source<u32, u32> = Source::from_array([1_u32, 2, 3]).map_materialized_value(|_| 10_u32);

  let values = source
    .flat_map_prefix_mat(1, |_prefix: Vec<u32>| Flow::<u32, u32, StreamNotUsed>::new(), KeepLeft)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // flat_map_prefix consumes prefix (1 element), then passes rest through the inner flow
  assert_eq!(values, vec![2_u32, 3]);
}

// --- r#async() ---

#[test]
fn source_async_passes_single_element_through() {
  // Given: a source emitting a single element
  // When: applying an async boundary on the source
  let values = Source::single(5_u32).r#async().run_with_collect_sink().expect("run_with_collect_sink");

  // Then: the element is forwarded unchanged
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn source_async_passes_multiple_elements_through() {
  // Given: a source emitting multiple elements
  let values =
    Source::from_array([1_u32, 2, 3, 4, 5]).r#async().run_with_collect_sink().expect("run_with_collect_sink");

  // Then: all elements arrive in order
  assert_eq!(values, vec![1_u32, 2, 3, 4, 5]);
}

#[test]
fn source_async_handles_empty_source() {
  // Given: an empty source
  let values = Source::<u32, _>::empty().r#async().run_with_collect_sink().expect("run_with_collect_sink");

  // Then: no elements are emitted, stream completes normally
  assert!(values.is_empty());
}

#[test]
fn source_async_composes_with_via() {
  use crate::core::dsl::Flow;

  // Given: a source with async boundary, then via a map flow
  let values = Source::from_array([10_u32, 20, 30])
    .r#async()
    .via(Flow::new().map(|x: u32| x + 1))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: async boundary + map compose correctly
  assert_eq!(values, vec![11_u32, 21, 31]);
}

#[test]
fn source_async_chained_multiple_boundaries() {
  // Given: a source with two chained async boundaries
  let values =
    Source::from_array([1_u32, 2, 3]).r#async().r#async().run_with_collect_sink().expect("run_with_collect_sink");

  // Then: elements pass through both boundaries in order
  assert_eq!(values, vec![1_u32, 2, 3]);
}

// --- B-1: Source::r#async() per-node attribute propagation ---

#[test]
fn source_async_marks_source_node_with_async_attribute_in_plan() {
  // Given: a source with async boundary → sink
  let source = Source::single(1_u32).r#async();

  // When: converting to a complete pipeline and plan
  let (mut graph, _) = source.into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // Then: the source stage has async boundary attribute
  assert!(plan.stages[0].attributes().is_async());
}

#[test]
fn source_async_does_not_affect_downstream_stages() {
  // Given: source.async() → map → sink
  let source = Source::single(1_u32).r#async().map(|x: u32| x + 1);

  // When: converting to a complete pipeline and plan
  let (mut graph, _) = source.into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // Then: source has async, map does not
  assert!(plan.stages[0].attributes().is_async());
  assert!(!plan.stages[1].attributes().is_async());
}

#[test]
fn source_async_with_dispatcher_marks_node_with_dispatcher_attribute() {
  // Given: a source with async + dispatcher → sink
  let source = Source::single(1_u32).async_with_dispatcher("custom-dispatcher");

  // When: converting to a complete pipeline and plan
  let (mut graph, _) = source.into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // Then: the source stage has both async and dispatcher attributes
  let attrs = plan.stages[0].attributes();
  assert!(attrs.is_async());
  let dispatcher = attrs.get::<DispatcherAttribute>();
  assert!(dispatcher.is_some());
  assert_eq!(dispatcher.unwrap().name(), "custom-dispatcher");
}

// ---------------------------------------------------------------------------
// Source DSL ミラー: also_to / also_to_mat / also_to_all
// ---------------------------------------------------------------------------

#[test]
fn source_also_to_passes_main_path_elements_through() {
  // Given: single-element source に also_to(Sink::ignore) を装着
  let values = Source::single(1_u32)
    .map(|value: u32| value + 1)
    .also_to(Sink::ignore())
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: main path では変換後の値が保持される
  assert_eq!(values, vec![2_u32]);
}

#[test]
fn source_also_to_mat_combines_materialized_values() {
  // Given: also_to_mat(Sink::head(), KeepBoth) の materialized
  let (graph, (left_mat, right_mat)) = Source::<u32, _>::empty().also_to_mat(Sink::head(), KeepBoth).into_parts();
  let _ = graph;

  // Then: 左は StreamNotUsed、右は Sink::head() の Pending completion
  assert_eq!(left_mat, StreamNotUsed::new());
  assert_eq!(right_mat.poll(), Completion::Pending);
}

#[test]
fn source_also_to_mat_keeps_main_path_behavior() {
  // Given: also_to_mat(Sink::ignore(), KeepBoth) を挿入
  let values = Source::single(1_u32)
    .map(|value: u32| value + 1)
    .also_to_mat(Sink::ignore(), KeepBoth)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: main path は map 後の値を出す
  assert_eq!(values, vec![2_u32]);
}

#[test]
fn source_also_to_mat_routes_elements_to_side_sink() {
  // Given: also_to_mat(Sink::head(), KeepRight) で右の completion を取り出す
  let (mut graph, side_completion) = Source::single(9_u32).also_to_mat(Sink::head(), KeepRight).into_parts();
  let (sink_graph, downstream_completion) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");
  let mut idle_budget = 1024_usize;

  // When: stream を完了まで駆動
  while !stream.state().is_terminal() {
    match stream.drive() {
      | DriveOutcome::Progressed => idle_budget = 1024,
      | DriveOutcome::Idle => {
        assert!(idle_budget > 0, "stream stalled");
        idle_budget = idle_budget.saturating_sub(1);
      },
    }
  }

  // Then: side sink は 9 を受け取り、main path は StreamDone で終わる
  assert_eq!(side_completion.poll(), Completion::Ready(Ok(9_u32)));
  assert_eq!(downstream_completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn source_also_to_all_passes_main_path_elements_through() {
  // Given: also_to_all で 2 本の sink を接続
  let sinks: Vec<Sink<u32, StreamCompletion<StreamDone>>> = alloc::vec![Sink::ignore(), Sink::ignore()];
  let values =
    Source::from_array([3_u32, 4_u32]).also_to_all(sinks).run_with_collect_sink().expect("run_with_collect_sink");

  // Then: main path は元の値をそのまま出す
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn source_also_to_all_multiple_sinks_each_receive_elements_exactly_once() {
  // Given: also_to_all で 3 本の sink を接続
  // 単一 Broadcast(4) を使うため各 sink へは 1 回だけ clone される（linear）。
  let first = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let first_clone = first.clone();
  let second = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let second_clone = second.clone();
  let third = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let third_clone = third.clone();
  let sinks = alloc::vec![
    Sink::foreach(move |value: u32| first_clone.lock().push(value)),
    Sink::foreach(move |value: u32| second_clone.lock().push(value)),
    Sink::foreach(move |value: u32| third_clone.lock().push(value)),
  ];
  let values =
    Source::from_array([10_u32, 20_u32]).also_to_all(sinks).run_with_collect_sink().expect("run_with_collect_sink");

  // Then: main path と 3 つの side sink がそれぞれ重複なく全要素を 1 回だけ受け取る
  assert_eq!(values, vec![10_u32, 20_u32]);
  assert_eq!(*first.lock(), vec![10_u32, 20_u32]);
  assert_eq!(*second.lock(), vec![10_u32, 20_u32]);
  assert_eq!(*third.lock(), vec![10_u32, 20_u32]);
}

// ---------------------------------------------------------------------------
// Source DSL ミラー: divert_to / divert_to_mat
// ---------------------------------------------------------------------------

#[test]
fn source_divert_to_routes_matching_and_passes_rest() {
  // Given: predicate で偶数を divert、奇数は main path
  let diverted = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let diverted_ref = diverted.clone();
  let values = Source::from_array([1_u32, 2_u32, 3_u32, 4_u32])
    .divert_to(
      |value: &u32| (*value).is_multiple_of(2),
      Sink::<u32, StreamCompletion<StreamDone>>::foreach(move |value| {
        diverted_ref.lock().push(value);
      }),
    )
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: main path は奇数、divert 側は偶数を受け取る
  assert_eq!(values, vec![1_u32, 3_u32]);
  assert_eq!(*diverted.lock(), vec![2_u32, 4_u32]);
}

#[test]
fn source_divert_to_mat_combines_materialized_values() {
  // Given: divert_to_mat で sink の materialized 値 23 を右に束ねる
  let sink = Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| 23_u32);
  let source = Source::<u32, StreamNotUsed>::empty().map_materialized_value(|_| 19_u32).divert_to_mat(
    |value: &u32| (*value).is_multiple_of(2),
    sink,
    KeepRight,
  );

  // When: 取り出し
  let (_graph, materialized) = source.into_parts();

  // Then: KeepRight は右の 23 を保持
  assert_eq!(materialized, 23_u32);
}

#[test]
fn source_divert_to_mat_preserves_main_path_behavior() {
  // Given: divert_to_mat(KeepLeft) で main path の偶数を排除
  let values = Source::from_array([1_u32, 2_u32, 3_u32, 4_u32])
    .divert_to_mat(
      |value: &u32| (*value).is_multiple_of(2),
      Sink::<u32, StreamCompletion<StreamDone>>::ignore().map_materialized_value(|_| 1_u32),
      KeepLeft,
    )
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: main path は奇数のみ
  assert_eq!(values, vec![1_u32, 3_u32]);
}

// ---------------------------------------------------------------------------
// Source DSL ミラー: or_else / or_else_mat
// ---------------------------------------------------------------------------

#[test]
fn source_or_else_uses_secondary_when_primary_is_empty() {
  // Given: 空の primary + 非空の secondary
  let values = Source::<u32, _>::empty()
    .or_else(Source::from_array([5_u32, 6_u32]))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: secondary の値が出る
  assert_eq!(values, vec![5_u32, 6_u32]);
}

#[test]
fn source_or_else_ignores_secondary_when_primary_emits() {
  // Given: 非空 primary + 非空 secondary
  let values = Source::from_array([7_u32, 8_u32])
    .or_else(Source::from_array([1_u32, 2_u32]))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: primary の値のみ
  assert_eq!(values, vec![7_u32, 8_u32]);
}

#[test]
fn source_or_else_mat_combines_materialized_values() {
  // Given: KeepBoth で primary / secondary の materialized を両方保持
  let secondary = Source::single(9_u32).map_materialized_value(|_| 17_u32);
  let source = Source::<u32, StreamNotUsed>::empty().map_materialized_value(|_| 3_u32).or_else_mat(secondary, KeepBoth);

  // When: 取り出し
  let (_graph, materialized) = source.into_parts();

  // Then: tuple が返る
  assert_eq!(materialized, (3_u32, 17_u32));
}

#[test]
fn source_or_else_mat_preserves_main_path_behavior() {
  // Given: empty primary + 非空 secondary + KeepLeft
  let values = Source::<u32, _>::empty()
    .or_else_mat(Source::from_array([5_u32, 6_u32]).map_materialized_value(|_| 77_u32), KeepLeft)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: secondary の要素が流れる
  assert_eq!(values, vec![5_u32, 6_u32]);
}

// ---------------------------------------------------------------------------
// Source DSL ミラー: watch_termination (非 mat、keep-left)
// ---------------------------------------------------------------------------

#[test]
fn source_watch_termination_passes_elements_through() {
  // Given: single-element source に watch_termination を装着
  let values = Source::single(42_u32).watch_termination().run_with_collect_sink().expect("run_with_collect_sink");

  // Then: 要素は透過される
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn source_watch_termination_keeps_left_materialized_value() {
  // Given: primary の materialized を 7 にして watch_termination を装着
  let (_graph, materialized) =
    Source::<u32, StreamNotUsed>::empty().map_materialized_value(|_| 7_u32).watch_termination().into_parts();

  // Then: KeepLeft 相当で元の 7 が残る
  assert_eq!(materialized, 7_u32);
}

// ---------------------------------------------------------------------------
// Source DSL ミラー: aggregate_with_boundary / batch_weighted /
//                  conflate / conflate_with_seed / expand / extrapolate
// ---------------------------------------------------------------------------

#[test]
fn source_aggregate_with_boundary_emits_fixed_size_chunks() {
  // Given: 5 要素の Source に aggregate_with_boundary(2) を装着
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]))
    .aggregate_with_boundary(2)
    .expect("aggregate_with_boundary")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: batch と同じく size=2 の Vec チャンクで出力される（残余を含む）
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32], vec![5_u32]]);
}

#[test]
fn source_aggregate_with_boundary_rejects_zero_size() {
  // Given: size=0 を指定
  let result = Source::single(1_u32).aggregate_with_boundary(0);

  // Then: validate_positive_argument により InvalidArgument(name="size") を返す
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_batch_weighted_uses_weight_budget() {
  // Given: weight=値そのものとして max_weight=3 を指定（Flow 側 batch_weighted_uses_weight_budget
  // と等価）
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[2, 1, 2]))
    .batch_weighted(3, |value| *value as usize)
    .expect("batch_weighted")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: weight 合計が 3 を超えない範囲で詰めて出力される
  assert_eq!(values, vec![vec![2_u32, 1_u32], vec![2_u32]]);
}

#[test]
fn source_batch_weighted_rejects_zero_max_weight() {
  // Given: max_weight=0 を指定
  let result = Source::single(1_u32).batch_weighted(0, |value: &u32| *value as usize);

  // Then: validate_positive_argument により InvalidArgument(name="max_weight") を返す
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "max_weight", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn source_conflate_preserves_elements_when_upstream_is_not_bursty() {
  // Given: 非バースティな入力 [1, 2, 3] に conflate を装着
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .conflate(|acc, value| acc + value)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 集約は発生せず、各要素がそのまま流れる（Flow 側等価テスト準拠）
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_conflate_aggregates_bursty_upstream_values() {
  // Given: map_concat で各値をバースト [v, v*10] に展開し、その後 conflate
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .map_concat(|value: u32| alloc::vec![value, value.saturating_mul(10)])
    .conflate(|acc, value| acc + value)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 同一バースト内の値が合算される（1+10=11, 2+20=22）
  assert_eq!(values, vec![11_u32, 22_u32]);
}

#[test]
fn source_conflate_with_seed_applies_seed_and_aggregate() {
  // Given: 非バースティな入力 [1, 2, 3] に conflate_with_seed(+10, +) を装着
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .conflate_with_seed(|value| value + 10, |acc, value| acc + value)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 各要素は seed のみ適用され（+10）、集約は発生しない
  assert_eq!(values, vec![11_u32, 12_u32, 13_u32]);
}

#[test]
fn source_conflate_with_seed_aggregates_bursty_upstream_values() {
  // Given: バースト化した上で conflate_with_seed(+100, +) を適用（Flow 側等価テスト準拠）
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .map_concat(|value: u32| alloc::vec![value, value.saturating_mul(10)])
    .conflate_with_seed(|value| value + 100, |acc, value| acc + value)
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 各バーストの先頭に seed を適用し、後続を集約する（101+10=111, 102+20=122）
  assert_eq!(values, vec![111_u32, 122_u32]);
}

#[test]
fn source_expand_emits_initial_values_for_each_input() {
  // Given: 各値を [v, v*10] に展開する expand を装着
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .expand(|value: &u32| alloc::vec![*value, value.saturating_mul(10)])
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: idle が無い場合は各入力の先頭値のみが出力される（Flow 側
  // expand_and_extrapolate_share_expand_behavior 準拠）
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn source_extrapolate_shares_expand_behavior() {
  // Given: 同一入力に対して expand と extrapolate を別々に適用
  let expand_values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .expand(|value: &u32| alloc::vec![*value, value.saturating_mul(10)])
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  let extrapolate_values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2]))
    .extrapolate(|value: &u32| alloc::vec![*value, value.saturating_mul(10)])
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: extrapolate は expand と同一挙動（Flow 側等価テスト準拠）
  assert_eq!(expand_values, vec![1_u32, 2_u32]);
  assert_eq!(expand_values, extrapolate_values);
}

// ---------------------------------------------------------------------------
// Batch 7 — Task S: 12 Source DSL operator mirrors
// ---------------------------------------------------------------------------
//
// Flow 側で既に確立されたシグネチャ（引数順・命名・`Result` 形・型境界）を
// Source 側でも完全に踏襲する。境界値検証（`validate_positive_argument`）が
// Flow 側にあるものは Source 側でも同一エラーを返すこと。タイムアウト発火は
// Flow 側で既に PulsedSourceLogic による統合テストがあるため、Source 側は
// happy path と zero rejection に絞る（Batch 6 の DropNew と同方針）。

// --- backpressure_timeout ---

#[test]
fn source_backpressure_timeout_keeps_single_path_behavior() {
  // Given: 3 要素シーケンスに十分余裕のある ticks を付けた backpressure_timeout を適用
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .backpressure_timeout(100)
    .expect("backpressure_timeout")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 全要素がそのまま通過する
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_backpressure_timeout_rejects_zero_ticks() {
  // Given: ticks=0 を渡す
  let result = Source::single(1_u32).backpressure_timeout(0);

  // Then: Flow 側と同じエラーシェイプで失敗する
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- completion_timeout ---

#[test]
fn source_completion_timeout_keeps_single_path_behavior() {
  // Given: 3 要素シーケンスに十分余裕のある ticks を付けた completion_timeout を適用
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .completion_timeout(100)
    .expect("completion_timeout")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 全要素がそのまま通過する
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_completion_timeout_rejects_zero_ticks() {
  // Given: ticks=0 を渡す
  let result = Source::single(1_u32).completion_timeout(0);

  // Then: Flow 側と同じエラーシェイプで失敗する
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- idle_timeout ---

#[test]
fn source_idle_timeout_keeps_single_path_behavior() {
  // Given: 3 要素シーケンスに十分余裕のある ticks を付けた idle_timeout を適用
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .idle_timeout(100)
    .expect("idle_timeout")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 全要素がそのまま通過する
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_idle_timeout_rejects_zero_ticks() {
  // Given: ticks=0 を渡す
  let result = Source::single(1_u32).idle_timeout(0);

  // Then: Flow 側と同じエラーシェイプで失敗する
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- initial_timeout ---

#[test]
fn source_initial_timeout_keeps_single_path_behavior() {
  // Given: 3 要素シーケンスに十分余裕のある ticks を付けた initial_timeout を適用
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .initial_timeout(100)
    .expect("initial_timeout")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 全要素がそのまま通過する
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_initial_timeout_rejects_zero_ticks() {
  // Given: ticks=0 を渡す
  let result = Source::single(1_u32).initial_timeout(0);

  // Then: Flow 側と同じエラーシェイプで失敗する
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- keep_alive ---

#[test]
fn source_keep_alive_keeps_single_path_behavior() {
  // Given: 3 要素シーケンスに十分余裕のある ticks を付けた keep_alive を適用
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .keep_alive(100, 0_u32)
    .expect("keep_alive")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: idle が発生しない場合、元の 3 要素がそのまま通過する
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_keep_alive_rejects_zero_ticks() {
  // Given: ticks=0 を渡す
  let result = Source::single(1_u32).keep_alive(0, 0_u32);

  // Then: Flow 側と同じエラーシェイプで失敗する
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

// --- wire_tap ---

#[test]
fn source_wire_tap_observes_each_element_without_altering_data_path() {
  // Given: 共有可変バッファを捕捉するコールバックを wire_tap に登録
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let observed_clone = observed.clone();

  // When: main path に 3 要素を流す
  let values = Source::from_array([10_u32, 20_u32, 30_u32])
    .wire_tap(move |value| {
      observed_clone.lock().push(*value);
    })
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: main path は変更されず、tap も全要素を観測する
  assert_eq!(values, vec![10_u32, 20_u32, 30_u32]);
  assert_eq!(*observed.lock(), vec![10_u32, 20_u32, 30_u32]);
}

// --- monitor ---

#[test]
fn source_monitor_emits_indexed_pairs_for_each_element() {
  // Given: 3 要素シーケンスに monitor を適用
  let values =
    Source::from_array([100_u32, 200_u32, 300_u32]).monitor().run_with_collect_sink().expect("run_with_collect_sink");

  // Then: 各要素に 0 始まりのインデックスが付与され (index, value) タプルが流れる
  assert_eq!(values, vec![(0_u64, 100_u32), (1_u64, 200_u32), (2_u64, 300_u32)]);
}

// --- log ---

#[test]
fn source_log_passes_elements_through_unchanged() {
  // Given: 3 要素シーケンスに log を適用
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .log("source-log")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 全要素がそのまま通過する
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_log_inserts_flow_log_stage_and_stores_name_attribute() {
  // Given: log を適用した Source をパイプライン化
  let source = Source::single(7_u32).log("source-log-stage");
  let (mut graph, _mat) = source.into_parts();

  // When: プランに展開して stage の属性を確認する
  let attribute_names = graph.attributes().names().to_vec();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  graph.append(sink_graph);
  let plan = graph.into_plan().expect("into_plan");

  // Then: FlowLog stage が 1 つ挿入され、名前属性が記録されている
  assert!(
    plan.stages.iter().any(|stage| matches!(
      stage,
      StageDefinition::Flow(definition) if definition.kind == StageKind::FlowLog
    )),
    "expected FlowLog stage in plan"
  );
  assert!(
    attribute_names.iter().any(|name| name == "source-log-stage"),
    "expected source-log-stage name attribute, got {attribute_names:?}"
  );
}

// --- log_with_marker ---

#[test]
fn source_log_with_marker_passes_elements_through_unchanged() {
  // Given: 3 要素シーケンスに log_with_marker を適用
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .log_with_marker("source-log", "marker")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 全要素がそのまま通過する
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn source_log_with_marker_stores_both_name_and_marker_attributes() {
  // Given: log_with_marker を適用した Source をグラフ化
  let source = Source::single(7_u32).log_with_marker("source-log-stage", "source-marker");
  let (graph, _mat) = source.into_parts();

  // Then: 属性リストに name / marker の双方が記録されている（Flow 側と同順）
  let attribute_names = graph.attributes().names().to_vec();
  assert!(
    attribute_names.iter().any(|name| name == "source-log-stage"),
    "expected source-log-stage name attribute, got {attribute_names:?}"
  );
  assert!(
    attribute_names.iter().any(|name| name == "source-marker"),
    "expected source-marker marker attribute, got {attribute_names:?}"
  );
}

// --- switch_map ---

#[test]
fn source_switch_map_emits_inner_source_values_for_each_outer_element() {
  // Given: 外側の 2 要素に対し、内側で single Source を生成する switch_map を適用
  let values = Source::from_array([1_u32, 2_u32])
    .switch_map(|value: u32| Source::single(value.saturating_mul(10)))
    .expect("switch_map")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 各外側要素の内側 Source の値が順番に流れる
  assert_eq!(values, vec![10_u32, 20_u32]);
}

// --- merge_latest ---

#[test]
fn source_merge_latest_wraps_single_path_value_into_vec() {
  // Given: fan_in=1 の merge_latest を single Source に適用
  let values = Source::single(7_u32)
    .merge_latest(1)
    .expect("merge_latest")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 単一要素が Vec でラップされて流れる（Flow 側等価テスト準拠）
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn source_merge_latest_rejects_zero_fan_in() {
  // Given: fan_in=0 を渡す
  let result = Source::single(1_u32).merge_latest(0);

  // Then: Flow 側と同じエラーシェイプで失敗する
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "fan_in", value: 0, reason: "must be greater than zero" })
  ));
}

// --- merge_preferred ---

#[test]
fn source_merge_preferred_keeps_single_path_behavior() {
  // Given: fan_in=1 の merge_preferred を single Source に適用
  let values = Source::single(7_u32)
    .merge_preferred(1)
    .expect("merge_preferred")
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // Then: 単一要素がそのまま流れる（Flow 側等価テスト準拠）
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_merge_preferred_rejects_zero_fan_in() {
  // Given: fan_in=0 を渡す
  let result = Source::single(1_u32).merge_preferred(0);

  // Then: Flow 側と同じエラーシェイプで失敗する
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "fan_in", value: 0, reason: "must be greater than zero" })
  ));
}
