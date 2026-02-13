use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::super::super::lifecycle::{Stream, StreamSharedGeneric};
use crate::core::{
  Completion, KeepRight, StreamBufferConfig, StreamError,
  hub::BroadcastHub,
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
fn broadcast_hub_delivers_to_all_subscribers() {
  let hub = BroadcastHub::new();
  let left = hub.subscribe();
  let right = hub.subscribe();

  hub.publish(10_u32).expect("publish 10");
  hub.publish(20_u32).expect("publish 20");

  assert_eq!(hub.poll(left), Some(10_u32));
  assert_eq!(hub.poll(left), Some(20_u32));
  assert_eq!(hub.poll(right), Some(10_u32));
  assert_eq!(hub.poll(right), Some(20_u32));
}

#[test]
fn broadcast_hub_source_for_drains_subscriber_queue() {
  let hub = BroadcastHub::new();
  let left = hub.subscribe();
  let _right = hub.subscribe();
  hub.publish(1_u32).expect("publish 1");
  hub.publish(2_u32).expect("publish 2");
  let mut materializer = TestMaterializer::default();

  let first_graph = hub.source_for(left).to_mat(Sink::head(), KeepRight);
  let first = first_graph.run(&mut materializer).expect("first materialize");
  for _ in 0..4 {
    let _ = first.handle().drive();
    if first.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(first.materialized().poll(), Completion::Ready(Ok(1_u32)));

  let second_graph = hub.source_for(left).to_mat(Sink::head(), KeepRight);
  let second = second_graph.run(&mut materializer).expect("second materialize");
  for _ in 0..4 {
    let _ = second.handle().drive();
    if second.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(second.materialized().poll(), Completion::Ready(Ok(2_u32)));
}

#[test]
fn broadcast_hub_source_waits_for_later_publish_without_completing() {
  let hub = BroadcastHub::new();
  let left = hub.subscribe();
  let graph = hub.source_for(left).to_mat(Sink::head(), KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");

  for _ in 0..3 {
    let _ = materialized.handle().drive();
  }
  assert_eq!(materialized.handle().state(), StreamState::Running);
  assert_eq!(materialized.materialized().poll(), Completion::Pending);

  hub.publish(55_u32).expect("publish 55");
  for _ in 0..4 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(materialized.handle().state(), StreamState::Completed);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(55_u32)));
}

#[test]
fn broadcast_hub_backpressures_when_no_subscriber_exists() {
  let hub = BroadcastHub::<u32>::new();
  assert_eq!(hub.publish(1_u32), Err(StreamError::WouldBlock));
}
