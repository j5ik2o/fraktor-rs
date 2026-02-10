use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::super::{stream::Stream, stream_shared::StreamSharedGeneric};
use crate::core::{
  Completion, KeepRight, Materialized, Materializer, PartitionHub, RunnableGraph, Sink, StreamBufferConfig,
  StreamError, StreamHandleGeneric, StreamHandleId, StreamState,
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
fn partition_hub_routes_values_to_selected_partitions() {
  let hub = PartitionHub::new(2);
  hub.offer(0, 1_u32);
  hub.offer(1, 2_u32);
  hub.offer(0, 3_u32);

  assert_eq!(hub.poll(0), Some(1_u32));
  assert_eq!(hub.poll(0), Some(3_u32));
  assert_eq!(hub.poll(1), Some(2_u32));
}

#[test]
fn partition_hub_source_for_drains_selected_partition() {
  let hub = PartitionHub::new(2);
  hub.offer(0, 7_u32);
  hub.offer(1, 8_u32);
  hub.offer(0, 9_u32);
  let mut materializer = TestMaterializer::default();

  let first_graph = hub.source_for(0).to_mat(Sink::head(), KeepRight);
  let first = first_graph.run(&mut materializer).expect("first materialize");
  for _ in 0..4 {
    let _ = first.handle().drive();
    if first.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(first.materialized().poll(), Completion::Ready(Ok(7_u32)));

  let second_graph = hub.source_for(0).to_mat(Sink::head(), KeepRight);
  let second = second_graph.run(&mut materializer).expect("second materialize");
  for _ in 0..4 {
    let _ = second.handle().drive();
    if second.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(second.materialized().poll(), Completion::Ready(Ok(9_u32)));
}

#[test]
fn partition_hub_source_waits_for_later_offer_without_completing() {
  let hub = PartitionHub::new(2);
  let graph = hub.source_for(1).to_mat(Sink::head(), KeepRight);
  let mut materializer = TestMaterializer::default();
  let materialized = graph.run(&mut materializer).expect("materialize");

  for _ in 0..3 {
    let _ = materialized.handle().drive();
  }
  assert_eq!(materialized.handle().state(), StreamState::Running);
  assert_eq!(materialized.materialized().poll(), Completion::Pending);

  hub.offer(1, 77_u32);
  for _ in 0..4 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(materialized.handle().state(), StreamState::Completed);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(77_u32)));
}
