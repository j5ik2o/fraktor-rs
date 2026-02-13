use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::super::super::lifecycle::{Stream, StreamSharedGeneric};
use crate::core::{
  Completion, KeepRight, StreamBufferConfig, StreamError,
  hub::MergeHub,
  lifecycle::{StreamHandleGeneric, StreamHandleId, StreamState},
  mat::{Materialized, Materializer, RunnableGraph},
  stage::Sink,
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

#[test]
fn merge_hub_preserves_offer_order() {
  let hub = MergeHub::new();
  let _source = hub.source();
  hub.offer(1_u32).expect("offer 1");
  hub.offer(2_u32).expect("offer 2");
  hub.offer(3_u32).expect("offer 3");

  assert_eq!(hub.poll(), Some(1_u32));
  assert_eq!(hub.poll(), Some(2_u32));
  assert_eq!(hub.poll(), Some(3_u32));
  assert_eq!(hub.poll(), None);
}

#[test]
fn merge_hub_source_drains_as_stream_source() {
  let hub = MergeHub::new();
  let _source = hub.source();
  hub.offer(10_u32).expect("offer 10");
  hub.offer(20_u32).expect("offer 20");
  let mut materializer = TestMaterializer::default();

  let first_graph = hub.source().to_mat(Sink::head(), KeepRight);
  let first = first_graph.run(&mut materializer).expect("first materialize");
  for _ in 0..4 {
    let _ = first.handle().drive();
    if first.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(first.materialized().poll(), Completion::Ready(Ok(10_u32)));

  let second_graph = hub.source().to_mat(Sink::head(), KeepRight);
  let second = second_graph.run(&mut materializer).expect("second materialize");
  for _ in 0..4 {
    let _ = second.handle().drive();
    if second.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(second.materialized().poll(), Completion::Ready(Ok(20_u32)));
}

#[test]
fn merge_hub_source_waits_for_later_offer_without_completing() {
  let hub = MergeHub::new();
  let graph = hub.source().to_mat(Sink::head(), KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");

  for _ in 0..3 {
    let _ = materialized.handle().drive();
  }
  assert_eq!(materialized.handle().state(), StreamState::Running);
  assert_eq!(materialized.materialized().poll(), Completion::Pending);

  hub.offer(42_u32).expect("offer 42");
  for _ in 0..4 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(materialized.handle().state(), StreamState::Completed);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(42_u32)));
}

#[test]
fn merge_hub_rejects_offer_until_receiver_is_activated() {
  let hub = MergeHub::new();

  let blocked = hub.offer(1_u32);
  assert_eq!(blocked, Err(StreamError::WouldBlock));

  let _source = hub.source();
  assert!(hub.offer(2_u32).is_ok());
  assert_eq!(hub.poll(), Some(2_u32));
}
