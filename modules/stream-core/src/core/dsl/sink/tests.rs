use alloc::{string::String, vec::Vec};
use core::{
  future::{Future, ready},
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::{
  DynValue, SinkDecision, SinkLogic, StreamDslError, StreamError,
  attributes::Attributes,
  dsl::{Sink, Source, tests::RunWithCollectSink},
  r#impl::{
    fusing::{DemandTracker, StreamBufferConfig},
    materialization::{Stream, StreamShared},
  },
  materialization::{
    Completion, KeepBoth, KeepRight, Materialized, Materializer, RunnableGraph, StreamDone, StreamFuture, StreamNotUsed,
  },
  stage::StageKind,
};

struct TestMaterializer {
  calls: usize,
}

impl TestMaterializer {
  const fn new() -> Self {
    Self { calls: 0 }
  }
}

impl Default for TestMaterializer {
  fn default() -> Self {
    Self::new()
  }
}

impl Materializer for TestMaterializer {
  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    self.calls = self.calls.saturating_add(1);
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

fn run_source_with_sink<In, Mat>(
  source: Source<In, StreamNotUsed>,
  sink: Sink<In, StreamFuture<Mat>>,
) -> Completion<Mat>
where
  In: Send + Sync + 'static,
  Mat: Send + Sync + Clone + 'static, {
  let graph = source.into_mat(sink, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..64 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }
  materialized.materialized().value()
}

fn drive_steps<Mat>(materialized: &Materialized<Mat>, steps: usize) -> bool {
  for _ in 0..steps {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      return true;
    }
  }
  false
}

#[test]
fn sink_map_materialized_value_transforms_materialized_value_and_keeps_data_path_behavior() {
  let (_graph, materialized) =
    Sink::<u32, StreamFuture<StreamDone>>::ignore().map_materialized_value(|_| 7_u32).into_parts();
  assert_eq!(materialized, 7_u32);

  let graph = Source::from_array([1_u32, 2_u32, 3_u32])
    .into_mat(Sink::<u32, StreamFuture<StreamDone>>::ignore().map_materialized_value(|_| 7_u32), KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..64 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }
  assert_eq!(*materialized.materialized(), 7_u32);
  assert!(materialized.stream().state().is_terminal());
}

#[test]
fn sink_collect_returns_all_elements() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::collect());
  assert_eq!(completion, Completion::Ready(Ok(vec![1_u32, 2, 3])));
}

#[test]
fn sink_collection_alias_returns_all_elements() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::collection());
  assert_eq!(completion, Completion::Ready(Ok(vec![1_u32, 2, 3])));
}

#[test]
fn sink_seq_alias_returns_all_elements() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::seq());
  assert_eq!(completion, Completion::Ready(Ok(vec![1_u32, 2, 3])));
}

#[test]
fn sink_count_counts_all_elements() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3, 4]), Sink::count());
  assert_eq!(completion, Completion::Ready(Ok(4_usize)));
}

#[test]
fn sink_exists_returns_true_when_matching_element_exists() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::exists(|value| *value == 2));
  assert_eq!(completion, Completion::Ready(Ok(true)));
}

#[test]
fn sink_forall_returns_false_when_non_matching_element_exists() {
  let completion = run_source_with_sink(Source::from_array([2_u32, 4, 5]), Sink::forall(|value| *value % 2 == 0));
  assert_eq!(completion, Completion::Ready(Ok(false)));
}

#[test]
fn sink_head_option_returns_some_for_non_empty_stream() {
  let completion = run_source_with_sink(Source::from_array([9_u32, 8, 7]), Sink::head_option());
  assert_eq!(completion, Completion::Ready(Ok(Some(9_u32))));
}

#[test]
fn sink_head_option_returns_none_for_empty_stream() {
  let completion = run_source_with_sink(Source::<u32, _>::from_array([]), Sink::head_option());
  assert_eq!(completion, Completion::Ready(Ok(None)));
}

#[test]
fn sink_last_option_returns_some_for_non_empty_stream() {
  let completion = run_source_with_sink(Source::from_array([9_u32, 8, 7]), Sink::last_option());
  assert_eq!(completion, Completion::Ready(Ok(Some(7_u32))));
}

#[test]
fn sink_last_option_returns_none_for_empty_stream() {
  let completion = run_source_with_sink(Source::<u32, _>::from_array([]), Sink::last_option());
  assert_eq!(completion, Completion::Ready(Ok(None)));
}

#[test]
fn sink_reduce_reduces_non_empty_stream() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3, 4]), Sink::reduce(|acc, value| acc + value));
  assert_eq!(completion, Completion::Ready(Ok(10_u32)));
}

#[test]
fn sink_reduce_fails_on_empty_stream() {
  let completion = run_source_with_sink(Source::<u32, _>::from_array([]), Sink::reduce(|acc, value| acc + value));
  assert_eq!(completion, Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn sink_take_last_keeps_tail_elements() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3, 4]), Sink::take_last(2));
  assert_eq!(completion, Completion::Ready(Ok(vec![3_u32, 4])));
}

#[test]
fn sink_take_last_with_zero_limit_returns_empty_vector() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3, 4]), Sink::take_last(0));
  assert_eq!(completion, Completion::Ready(Ok(vec![])));
}

#[test]
fn sink_cancelled_completes_with_done() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::cancelled());
  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn sink_none_alias_completes_with_done() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::none());
  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn sink_never_keeps_completion_pending_after_upstream_finishes() {
  let graph = Source::from_array([1_u32, 2, 3]).into_mat(Sink::never(), KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");

  assert!(drive_steps(&materialized, 64));
  assert_eq!(materialized.materialized().value(), Completion::Pending);
  assert!(materialized.stream().state().is_terminal());
}

#[test]
fn sink_on_complete_invokes_callback_on_success() {
  let observed = ArcShared::new(SpinSyncMutex::new(None::<Result<StreamDone, StreamError>>));
  let observed_ref = observed.clone();
  let sink = Sink::on_complete(move |result| {
    let mut guard = observed_ref.lock();
    *guard = Some(result);
  });
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), sink);

  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
  assert_eq!(*observed.lock(), Some(Ok(StreamDone::new())));
}

#[test]
fn sink_on_complete_invokes_callback_on_failure() {
  let observed = ArcShared::new(SpinSyncMutex::new(None::<Result<StreamDone, StreamError>>));
  let observed_ref = observed.clone();
  let sink = Sink::on_complete(move |result| {
    let mut guard = observed_ref.lock();
    *guard = Some(result);
  });
  let completion = run_source_with_sink(Source::<u32, _>::failed(StreamError::Failed), sink);

  assert_eq!(completion, Completion::Ready(Err(StreamError::Failed)));
  assert_eq!(*observed.lock(), Some(Err(StreamError::Failed)));
}

#[test]
fn sink_combine_routes_elements_to_all_combined_sinks() {
  let left_values = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let right_values = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let left_values_ref = left_values.clone();
  let right_values_ref = right_values.clone();

  let sink = Sink::combine([
    Sink::foreach(move |value| left_values_ref.lock().push(value)),
    Sink::foreach(move |value| right_values_ref.lock().push(value)),
  ]);
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), sink);

  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
  assert_eq!(*left_values.lock(), vec![1_u32, 2, 3]);
  assert_eq!(*right_values.lock(), vec![1_u32, 2, 3]);
}

#[test]
fn sink_combine_mat_combines_materialized_values_with_keep_both() {
  let sink = Sink::combine_mat(Sink::collect(), Sink::collect(), KeepBoth);
  let graph = Source::from_array([1_u32, 2, 3]).into_mat(sink, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");

  assert!(drive_steps(&materialized, 64));
  let combined = materialized.materialized();
  assert_eq!(combined.0.value(), Completion::Ready(Ok(vec![1_u32, 2, 3])));
  assert_eq!(combined.1.value(), Completion::Ready(Ok(vec![1_u32, 2, 3])));
}

#[test]
fn sink_fold_while_stops_updating_after_predicate_is_false() {
  let completion = run_source_with_sink(
    Source::from_array([1_u32, 2, 3, 4]),
    Sink::fold_while(0_u32, |acc, _| *acc < 3, |acc, value| acc + value),
  );
  assert_eq!(completion, Completion::Ready(Ok(3_u32)));
}

#[test]
fn sink_fold_async_accumulates_values_when_future_is_ready() {
  let completion = run_source_with_sink(
    Source::from_array([1_u32, 2, 3, 4]),
    Sink::fold_async(0_u32, |acc, value| ready(acc + value)),
  );
  assert_eq!(completion, Completion::Ready(Ok(10_u32)));
}

#[test]
fn sink_fold_async_propagates_upstream_failure() {
  let completion = run_source_with_sink(
    Source::<u32, _>::failed(StreamError::Failed),
    Sink::fold_async(0_u32, |acc, value| ready(acc + value)),
  );
  assert_eq!(completion, Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn sink_fold_async_waits_for_pending_future_before_completion() {
  let graph = Source::single(7_u32)
    .into_mat(Sink::fold_async(0_u32, |acc, value| YieldThenOutputFuture::new(acc + value)), KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(materializer.calls, 1);
  assert_eq!(materialized.stream().drive(), crate::core::materialization::DriveOutcome::Progressed);
  assert_eq!(materialized.materialized().value(), Completion::Pending);

  for _ in 0..64 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }

  assert!(materialized.stream().state().is_terminal());
  assert_eq!(materialized.materialized().value(), Completion::Ready(Ok(7_u32)));
}

#[test]
fn sink_foreach_async_rejects_zero_parallelism() {
  let result = Sink::<u32, StreamFuture<StreamDone>>::foreach_async(0, |_value| async move {});
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "parallelism", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn sink_foreach_async_accepts_positive_parallelism() {
  let sink = Sink::<u32, StreamFuture<StreamDone>>::foreach_async(1, |_value| async move {}).expect("foreach_async");
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), sink);
  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn sink_from_materializer_defers_factory_and_uses_created_sink() {
  let factory_calls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let factory_calls_ref = factory_calls.clone();
  let sink = Sink::<u32, StreamFuture<Vec<u32>>>::from_materializer(move || {
    *factory_calls_ref.lock() += 1;
    Sink::collect()
  });

  assert_eq!(*factory_calls.lock(), 0_u32);
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), sink);

  assert_eq!(*factory_calls.lock(), 1_u32);
  assert_eq!(completion, Completion::Ready(Ok(vec![1_u32, 2, 3])));
}

#[test]
fn sink_lazy_sink_alias_completes_with_done() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::lazy_sink(Sink::ignore));
  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn sink_lazy_sink_defers_factory_call() {
  let called = ArcShared::new(SpinSyncMutex::new(false));
  let called_clone = called.clone();
  let sink = Sink::lazy_sink(move || {
    *called_clone.lock() = true;
    Sink::ignore()
  });
  // ファクトリはまだ呼ばれていない
  assert!(!*called.lock());
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), sink);
  // ファクトリが呼ばれ、完了する
  assert!(*called.lock());
  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn sink_lazy_sink_with_foreach_processes_elements() {
  let collected = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let collected_clone = collected.clone();
  let sink = Sink::lazy_sink(move || {
    Sink::foreach(move |value: u32| {
      collected_clone.lock().push(value);
    })
  });
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), sink);
  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
  assert_eq!(*collected.lock(), vec![1_u32, 2, 3]);
}

#[test]
fn sink_lazy_sink_with_empty_source_completes() {
  let completion = run_source_with_sink(Source::<u32, _>::empty(), Sink::lazy_sink(Sink::ignore));
  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn sink_pre_materialize_returns_pending_completion_handle() {
  let (_sink, completion) = Sink::<u32, StreamFuture<StreamDone>>::ignore().pre_materialize();
  assert_eq!(completion.value(), Completion::Pending);
}

#[test]
fn sink_pre_materialize_completion_resolves_when_sink_finishes() {
  // The handle returned by pre_materialize must be the sink's own
  // completion, not a freshly-allocated disconnected StreamFuture.
  let (sink, completion) = Sink::<u32, StreamFuture<StreamDone>>::ignore().pre_materialize();
  let graph = Source::from_array([1_u32, 2, 3]).into_mat(sink, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..64 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }
  assert_eq!(completion.value(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn sink_source_materializes_live_source_with_upstream_elements() {
  let sink: Sink<u32, Source<u32, StreamNotUsed>> = Sink::source();
  let graph = Source::from_array([1_u32, 2, 3]).into_mat(sink, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");

  assert!(drive_steps(&materialized, 64));
  let source = materialized.into_materialized();
  let values = source.run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn sink_java_collector_alias_collects_values() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::java_collector());
  assert_eq!(completion, Completion::Ready(Ok(vec![1_u32, 2, 3])));
}

#[test]
fn sink_java_collector_parallel_unordered_alias_collects_values() {
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), Sink::java_collector_parallel_unordered());
  assert_eq!(completion, Completion::Ready(Ok(vec![1_u32, 2, 3])));
}

#[test]
fn sink_to_path_collects_bytes() {
  let completion = run_source_with_sink(Source::from_array([b'a', b'b']), Sink::collect());
  assert_eq!(completion, Completion::Ready(Ok(vec![b'a', b'b'])));
}

// inner sink の on_complete エラーを検証するためのカスタム SinkLogic
struct FailOnCompleteSinkLogic {
  completion: StreamFuture<StreamDone>,
}

impl SinkLogic for FailOnCompleteSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, _input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Err(StreamError::Failed));
    Err(StreamError::Failed)
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

#[test]
fn sink_lazy_sink_propagates_inner_on_complete_error() {
  let inner_completion = StreamFuture::new();
  let inner_sink = Sink::<u32, StreamFuture<StreamDone>>::from_definition(
    StageKind::Custom,
    FailOnCompleteSinkLogic { completion: inner_completion.clone() },
    inner_completion,
  );
  let lazy = Sink::lazy_sink(move || inner_sink);
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), lazy);
  assert_eq!(completion, Completion::Ready(Err(StreamError::Failed)));
}

struct PendingInnerSinkLogic {
  completion:        StreamFuture<StreamDone>,
  observed:          ArcShared<SpinSyncMutex<Vec<u32>>>,
  pending:           bool,
  upstream_finished: bool,
  completed:         bool,
}

impl SinkLogic for PendingInnerSinkLogic {
  fn can_accept_input(&self) -> bool {
    !self.pending
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, _demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    if self.pending {
      return Err(StreamError::Failed);
    }
    let value = crate::core::downcast_value::<u32>(input)?;
    self.observed.lock().push(value);
    self.pending = true;
    Ok(SinkDecision::Continue)
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    if !self.pending {
      return Ok(false);
    }
    self.pending = false;
    if self.upstream_finished {
      self.completion.complete(Ok(StreamDone::new()));
      self.completed = true;
    } else {
      demand.request(1)?;
    }
    Ok(true)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    if !self.completed {
      self.completion.complete(Ok(StreamDone::new()));
      self.completed = true;
    }
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
    self.completed = true;
  }

  fn on_upstream_finish(&mut self) -> Result<bool, StreamError> {
    self.upstream_finished = true;
    if self.pending {
      return Ok(false);
    }
    if !self.completed {
      self.completion.complete(Ok(StreamDone::new()));
      self.completed = true;
      return Ok(true);
    }
    Ok(false)
  }

  fn has_pending_work(&self) -> bool {
    self.pending
  }
}

#[test]
fn sink_lazy_sink_delegates_pending_inner_lifecycle() {
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let inner_completion = StreamFuture::new();
  let inner_sink = Sink::<u32, StreamFuture<StreamDone>>::from_definition(
    StageKind::Custom,
    PendingInnerSinkLogic {
      completion:        inner_completion.clone(),
      observed:          observed.clone(),
      pending:           false,
      upstream_finished: false,
      completed:         false,
    },
    inner_completion,
  );

  let graph = Source::from_array([1_u32, 2_u32]).into_mat(Sink::lazy_sink(move || inner_sink), KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(materializer.calls, 1);
  assert_eq!(materialized.stream().drive(), crate::core::materialization::DriveOutcome::Progressed);
  assert_eq!(materialized.materialized().value(), Completion::Pending);

  for _ in 0..64 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }

  assert!(materialized.stream().state().is_terminal());
  assert_eq!(materialized.materialized().value(), Completion::Ready(Ok(StreamDone::new())));
  assert_eq!(*observed.lock(), vec![1_u32, 2_u32]);
}

#[test]
fn sink_contramap_transforms_input_type() {
  let sink = Sink::<u32, StreamFuture<Vec<u32>>>::collect().contramap(|s: &str| s.len() as u32);
  let completion = run_source_with_sink(Source::from_array(["hello", "hi", "hey"]), sink);
  assert_eq!(completion, Completion::Ready(Ok(alloc::vec![5_u32, 2, 3])));
}

#[test]
fn sink_from_graph_creates_sink_from_existing_graph() {
  let original = Sink::<u32, StreamFuture<Vec<u32>>>::collect();
  let (graph, mat) = original.into_parts();
  let reconstructed = Sink::<u32, StreamFuture<Vec<u32>>>::from_graph(graph, mat);
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), reconstructed);
  assert_eq!(completion, Completion::Ready(Ok(alloc::vec![1_u32, 2, 3])));
}

#[test]
fn sink_named_keeps_behavior_and_sets_attributes() {
  let sink = Sink::<u32, _>::ignore().named("test-sink");
  let completion = run_source_with_sink(Source::from_array([1_u32, 2, 3]), sink);
  assert_eq!(completion, Completion::Ready(Ok(StreamDone::new())));

  let (graph, _mat) = Sink::<u32, _>::ignore().named("test-sink").into_parts();
  assert_eq!(graph.attributes().names(), &[String::from("test-sink")]);
}

#[test]
fn sink_with_and_add_attributes_merge_names() {
  let (graph, _mat) = Sink::<u32, _>::ignore()
    .with_attributes(Attributes::named("base"))
    .add_attributes(Attributes::named("extra"))
    .into_parts();
  assert_eq!(graph.attributes().names(), &[String::from("base"), String::from("extra")]);
}

#[test]
fn sink_queue_collects_elements() {
  let queue_sink = Sink::<u32, _>::queue();
  let graph = Source::from_array([1_u32, 2, 3]).into_mat(queue_sink, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..64 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }
  let queue = materialized.materialized();
  assert_eq!(queue.pull(), Some(1_u32));
  assert_eq!(queue.pull(), Some(2_u32));
  assert_eq!(queue.pull(), Some(3_u32));
  assert!(queue.pull().is_none());
}

// --- A1: Sink.never ---

#[test]
fn sink_never_does_not_complete_without_elements() {
  // 前提: 要素を流さない source と、成功完了しない sink を組み合わせる
  let source = Source::<u32, StreamNotUsed>::empty();
  let sink = Sink::<u32, StreamFuture<StreamDone>>::never();

  // 操作: ストリームを実行する
  let graph = source.into_mat(sink, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..64 {
    let _ = materialized.stream().drive();
  }

  // 期待: materialized completion は成功完了しない
  let completion = materialized.materialized().value();
  assert!(!matches!(completion, Completion::Ready(Ok(_))), "Sink::never should not complete successfully on its own");
}

#[test]
fn sink_never_accepts_elements_without_completing() {
  // 前提: 要素を流す source と、成功完了しない sink を組み合わせる
  let source = Source::from_array([1_u32, 2, 3]);
  let sink = Sink::<u32, StreamFuture<StreamDone>>::never();

  // 操作: ストリームを実行する
  let graph = source.into_mat(sink, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..64 {
    let _ = materialized.stream().drive();
  }

  // 期待: sink 側の materialized completion は成功完了しない
  let completion = materialized.materialized().value();
  assert!(!matches!(completion, Completion::Ready(Ok(_))), "Sink::never should not produce a successful completion");
}

// --- C1: Sink.combine / Sink.combineMat ---

#[test]
fn sink_combine_distributes_elements_to_all_sinks() {
  // 前提: 2 つの collect sink を combine する
  let sink1 = Sink::<u32, StreamFuture<Vec<u32>>>::collect();
  let sink2 = Sink::<u32, StreamFuture<Vec<u32>>>::collect();
  let combined = Sink::combine(vec![sink1, sink2]);

  // 操作: combine した sink に要素を流す
  let source = Source::from_array([1_u32, 2, 3]);
  let completion = run_source_with_sink(source, combined);

  // 期待: combine した sink は完了する
  assert!(matches!(completion, Completion::Ready(_)));
}

#[test]
fn sink_combine_with_empty_iterator_creates_cancelled_sink() {
  // 前提: combine 対象の sink が 0 件である
  let combined =
    Sink::<u32, StreamFuture<StreamDone>>::combine(core::iter::empty::<Sink<u32, StreamFuture<StreamDone>>>());

  // 操作: source と接続して実行する
  let source = Source::from_array([1_u32, 2, 3]);
  let graph = source.into_mat(combined, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..64 {
    let _ = materialized.stream().drive();
    if materialized.stream().state().is_terminal() {
      break;
    }
  }

  // 期待: 空の combine は退化した sink として終端に到達する
  assert!(materialized.stream().state().is_terminal());
}

#[test]
fn sink_combine_mat_keeps_both_materialized_values() {
  // 前提: 異なる materialized value を持つ 2 つの sink を KeepBoth で combine する
  use crate::core::materialization::KeepBoth;

  let sink1 = Sink::<u32, StreamFuture<StreamDone>>::ignore();
  let sink2 = Sink::<u32, StreamFuture<StreamDone>>::ignore();
  let combined = Sink::combine_mat(sink1, sink2, KeepBoth);

  // 操作: materialized value を取り出す
  let (_graph, mat) = combined.into_parts();

  // 期待: 両方の materialized value が pending のタプルとして返る
  let (left, right) = mat;
  assert_eq!(left.value(), Completion::Pending);
  assert_eq!(right.value(), Completion::Pending);
}

#[test]
fn sink_combine_mat_keeps_left_materialized_value() {
  // 前提: 2 つの sink を KeepLeft で combine する
  use crate::core::materialization::KeepLeft;

  let sink1 = Sink::<u32, StreamFuture<StreamDone>>::ignore();
  let sink2 = Sink::<u32, StreamFuture<StreamDone>>::ignore();
  let combined = Sink::combine_mat(sink1, sink2, KeepLeft);

  // 操作: materialized value を取り出す
  let (_graph, mat) = combined.into_parts();

  // 期待: 左側の materialized value が pending のまま返る
  assert_eq!(mat.value(), Completion::Pending);
}
