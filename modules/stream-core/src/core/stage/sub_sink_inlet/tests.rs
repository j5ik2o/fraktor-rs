use alloc::{vec, vec::Vec};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::SubSinkInlet;
use crate::core::{
  StreamError,
  dsl::Source,
  r#impl::{
    fusing::StreamBufferConfig,
    materialization::{Stream, StreamHandleId, StreamHandleImpl, StreamShared},
  },
  materialization::{KeepRight, Materialized, Materializer, RunnableGraph, StreamNotUsed},
  stage::SubSinkInletHandler,
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

struct RecordingSubSinkInletHandler {
  events: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl SubSinkInletHandler<u32> for RecordingSubSinkInletHandler {
  fn on_push(&mut self) -> Result<(), StreamError> {
    self.events.lock().push("push");
    Ok(())
  }

  fn on_upstream_finish(&mut self) -> Result<(), StreamError> {
    self.events.lock().push("finish");
    Ok(())
  }

  fn on_upstream_failure(&mut self, _error: StreamError) -> Result<(), StreamError> {
    self.events.lock().push("failure");
    Ok(())
  }
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
fn sink_receives_pulled_element_and_makes_it_available_to_grab() {
  // Given: pull 済みの SubSinkInlet と、その sink に流す Source
  let mut inlet = SubSinkInlet::<u32>::new("sub-sink");
  let events = ArcShared::new(SpinSyncMutex::new(Vec::<&'static str>::new()));
  inlet.set_handler(RecordingSubSinkInletHandler { events: events.clone() });
  let sink = inlet.sink();
  inlet.pull().expect("pull");

  // When: Source から sink へ 1 要素を流す
  let graph = Source::single(42_u32).into_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized = graph.run(&mut materializer).expect("materialize");
  drive_until_terminal(&materialized);

  // Then: handler が push/finish を観測し、grab で値を取り出せる
  assert_eq!(*materialized.materialized(), StreamNotUsed::new());
  assert_eq!(*events.lock(), vec!["push", "finish"]);
  assert!(inlet.is_available());
  assert!(inlet.is_closed());
  assert!(!inlet.has_been_pulled());
  assert_eq!(inlet.grab().expect("grab"), 42_u32);
  assert!(!inlet.is_available());
}

#[test]
fn grab_before_element_arrives_fails_without_closing_port() {
  // Given: まだ要素が到着していない SubSinkInlet
  let mut inlet = SubSinkInlet::<u32>::new("sub-sink");

  // When: grab を呼び出す
  let result = inlet.grab();

  // Then: no-op で None を返さず、失敗として観測できる
  assert_eq!(result, Err(StreamError::WouldBlock));
  assert!(!inlet.is_available());
  assert!(!inlet.is_closed());
}

#[test]
fn double_pull_fails_and_keeps_first_pull_state() {
  // Given: すでに pull 済みの SubSinkInlet
  let mut inlet = SubSinkInlet::<u32>::new("sub-sink");
  inlet.pull().expect("first pull");

  // When: 同じ port を再度 pull する
  let result = inlet.pull();

  // Then: Pekko と同じく二重 pull は失敗し、最初の pull 状態は維持される
  assert!(result.is_err());
  assert!(inlet.has_been_pulled());
  assert!(!inlet.is_closed());
}

#[test]
fn cancel_closes_port_and_rejects_later_pull() {
  // Given: pull 済みの SubSinkInlet
  let mut inlet = SubSinkInlet::<u32>::new("sub-sink");
  inlet.pull().expect("pull");

  // When: cancel する
  inlet.cancel().expect("cancel");

  // Then: port は閉じられ、追加 pull は失敗する
  assert!(inlet.is_closed());
  assert!(!inlet.has_been_pulled());
  assert!(inlet.pull().is_err());
}

#[test]
fn upstream_failure_closes_port_and_notifies_handler() {
  // Given: failure を吸収して記録する handler
  let mut inlet = SubSinkInlet::<u32>::new("sub-sink");
  let events = ArcShared::new(SpinSyncMutex::new(Vec::<&'static str>::new()));
  inlet.set_handler(RecordingSubSinkInletHandler { events: events.clone() });
  let sink = inlet.sink();
  inlet.pull().expect("pull");

  // When: upstream Source が失敗する
  let graph = Source::<u32, StreamNotUsed>::failed(StreamError::Failed).into_mat(sink, KeepRight);
  let mut materializer = TestMaterializer;
  let materialized: Materialized<StreamNotUsed> = graph.run(&mut materializer).expect("materialize");
  drive_until_terminal(&materialized);

  // Then: port は閉じられ、failure callback が呼び出される
  assert!(inlet.is_closed());
  assert!(!inlet.is_available());
  assert_eq!(*events.lock(), vec!["failure"]);
}

#[test]
fn sink_materialized_value_is_not_used() {
  // Given: SubSinkInlet が公開する sink
  let inlet = SubSinkInlet::<u32>::new("sub-sink");
  let sink = inlet.sink();

  // When: public Sink として materialized value を取り出す
  let (_graph, materialized) = sink.into_parts();

  // Then: Pekko の SubSinkInlet.sink と同じく追加 materialized value は持たない
  assert_eq!(materialized, StreamNotUsed::new());
}
