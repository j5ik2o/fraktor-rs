use fraktor_utils_rs::core::{
  runtime_toolbox::NoStdToolbox,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use super::super::super::lifecycle::{Stream, StreamSharedGeneric};
use crate::core::{
  Completion, KeepRight, StreamBufferConfig, StreamCompletion, StreamDone, StreamError, StreamNotUsed,
  lifecycle::{StreamHandleGeneric, StreamHandleId},
  mat::{Materialized, Materializer, RunnableGraph},
  stage::{Sink, Source},
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
  type Toolbox = NoStdToolbox;

  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat, Self::Toolbox>, StreamError> {
    self.calls = self.calls.saturating_add(1);
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

fn run_source_with_sink<In, Mat>(
  source: Source<In, StreamNotUsed>,
  sink: Sink<In, StreamCompletion<Mat>>,
) -> Completion<Mat>
where
  In: Send + Sync + 'static,
  Mat: Send + Sync + Clone + 'static, {
  let graph = source.to_mat(sink, KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..64 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      break;
    }
  }
  materialized.materialized().poll()
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
