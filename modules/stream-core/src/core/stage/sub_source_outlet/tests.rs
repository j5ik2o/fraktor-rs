use alloc::{vec, vec::Vec};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::SubSourceOutlet;
use crate::core::{
  StreamError,
  dsl::Sink,
  r#impl::{
    fusing::StreamBufferConfig,
    materialization::{Stream, StreamHandleId, StreamHandleImpl, StreamShared},
  },
  materialization::{
    Completion, KeepRight, Materialized, Materializer, RunnableGraph, StreamCompletion, StreamNotUsed,
  },
  stage::{CancellationCause, SubSourceOutletHandler},
};

struct TestMaterializer;

impl Materializer for TestMaterializer {
  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let shared = StreamShared::new(stream);
    let handle = StreamHandleImpl::new(StreamHandleId::next(), shared);
    Ok(Materialized::new(handle, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

struct RecordingSubSourceOutletHandler {
  events: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl SubSourceOutletHandler<u32> for RecordingSubSourceOutletHandler {
  fn on_pull(&mut self) -> Result<(), StreamError> {
    self.events.lock().push("pull");
    Ok(())
  }

  fn on_downstream_finish(&mut self, _cause: CancellationCause) -> Result<(), StreamError> {
    self.events.lock().push("finish");
    Ok(())
  }
}

fn drive_until<Mat, F>(materialized: &Materialized<Mat>, mut predicate: F)
where
  F: FnMut() -> bool, {
  for _ in 0..64 {
    if predicate() {
      return;
    }
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() && !predicate() {
      break;
    }
  }
  panic!("stream did not reach expected state");
}

fn drive_until_terminal<Mat>(materialized: &Materialized<Mat>) {
  for _ in 0..64 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      return;
    }
  }
  panic!("stream did not terminate");
}

#[test]
fn source_emits_pushed_elements_after_downstream_pull() {
  // Given: Sink::collect に接続した SubSourceOutlet
  let mut outlet = SubSourceOutlet::<u32>::new("sub-source");
  let events = ArcShared::new(SpinSyncMutex::new(Vec::<&'static str>::new()));
  outlet.set_handler(RecordingSubSourceOutletHandler { events: events.clone() });
  let graph = outlet.source().into_mat(Sink::<u32, StreamCompletion<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("materialize");

  // When: downstream demand を待ってから 2 要素を push し、完了する
  drive_until(&materialized, || outlet.is_available());
  outlet.push(10_u32).expect("first push");
  drive_until(&materialized, || outlet.is_available());
  outlet.push(20_u32).expect("second push");
  outlet.complete().expect("complete");
  drive_until_terminal(&materialized);

  // Then: push した要素が Source の data path に流れ、pull callback も観測される
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(vec![10_u32, 20_u32])));
  assert_eq!(*events.lock(), vec!["pull", "pull"]);
  assert!(outlet.is_closed());
  assert!(!outlet.is_available());
}

#[test]
fn push_before_downstream_pull_fails_without_closing_port() {
  // Given: まだ demand を受けていない SubSourceOutlet
  let mut outlet = SubSourceOutlet::<u32>::new("sub-source");

  // When: pull 前に push する
  let result = outlet.push(1_u32);

  // Then: no-op enqueue ではなく失敗し、port は開いたまま残る
  assert!(result.is_err());
  assert!(!outlet.is_available());
  assert!(!outlet.is_closed());
}

#[test]
fn push_after_complete_fails() {
  // Given: complete 済みの SubSourceOutlet
  let mut outlet = SubSourceOutlet::<u32>::new("sub-source");
  outlet.complete().expect("complete");

  // When: 完了後に push する
  let result = outlet.push(1_u32);

  // Then: closed port への push は失敗する
  assert!(outlet.is_closed());
  assert!(result.is_err());
}

#[test]
fn fail_propagates_error_to_source_consumer() {
  // Given: Sink::collect に接続した SubSourceOutlet
  let mut outlet = SubSourceOutlet::<u32>::new("sub-source");
  let graph = outlet.source().into_mat(Sink::<u32, StreamCompletion<Vec<u32>>>::collect(), KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("materialize");

  // When: downstream demand 到着後に fail する
  drive_until(&materialized, || outlet.is_available());
  outlet.fail(StreamError::Failed).expect("fail");
  drive_until_terminal(&materialized);

  // Then: source consumer は同じ StreamError を観測する
  assert!(outlet.is_closed());
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn source_materialized_value_is_not_used() {
  // Given: SubSourceOutlet が公開する source
  let outlet = SubSourceOutlet::<u32>::new("sub-source");
  let source = outlet.source();

  // When: public Source として materialized value を取り出す
  let (_graph, materialized) = source.into_parts();

  // Then: Pekko の SubSourceOutlet.source と同じく追加 materialized value は持たない
  assert_eq!(materialized, StreamNotUsed::new());
}
