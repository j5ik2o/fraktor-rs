use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::super::super::lifecycle::{Stream, StreamSharedGeneric};
use crate::core::{
  Completion, KeepRight, StreamBufferConfig, StreamError,
  hub::PartitionHub,
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
fn partition_hub_routes_values_to_selected_partitions() {
  let hub = PartitionHub::new(2);
  let _left = hub.source_for(0);
  let _right = hub.source_for(1);
  hub.offer(0, 1_u32).expect("offer left 1");
  hub.offer(1, 2_u32).expect("offer right 2");
  hub.offer(0, 3_u32).expect("offer left 3");

  assert_eq!(hub.poll(0), Some(1_u32));
  assert_eq!(hub.poll(0), Some(3_u32));
  assert_eq!(hub.poll(1), Some(2_u32));
}

#[test]
fn partition_hub_source_for_drains_selected_partition() {
  let hub = PartitionHub::new(2);
  let _left = hub.source_for(0);
  let _right = hub.source_for(1);
  hub.offer(0, 7_u32).expect("offer left 7");
  hub.offer(1, 8_u32).expect("offer right 8");
  hub.offer(0, 9_u32).expect("offer left 9");
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

  hub.offer(1, 77_u32).expect("offer 77");
  for _ in 0..4 {
    let _ = materialized.handle().drive();
    if materialized.handle().state().is_terminal() {
      break;
    }
  }
  assert_eq!(materialized.handle().state(), StreamState::Completed);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(77_u32)));
}

#[test]
fn partition_hub_rejects_offer_when_partition_has_no_active_consumer() {
  let hub = PartitionHub::new(2);
  assert_eq!(hub.offer(0, 1_u32), Err(StreamError::WouldBlock));

  let _source = hub.source_for(0);
  assert!(hub.offer(0, 2_u32).is_ok());
  assert_eq!(hub.poll(0), Some(2_u32));
}

#[test]
fn partition_hub_route_with_partitioner_assigns_to_active_consumer() {
  let hub = PartitionHub::new(3);
  let _consumer_zero = hub.source_for(0);
  let _consumer_two = hub.source_for(2);

  hub
    .route_with(99_u32, |consumer_count| {
      assert_eq!(consumer_count, 2);
      1 // Nth-active-consumer index: 0 → partition 0, 1 → partition 2
    })
    .expect("route_with");

  assert_eq!(hub.poll(2), Some(99_u32));
  assert_eq!(hub.poll(0), None);
}

#[test]
fn partition_hub_route_with_partitioner_rejects_invalid_route() {
  let hub = PartitionHub::new(2);
  let _consumer_zero = hub.source_for(0);

  let negative = hub.route_with(1_u32, |_| -1);
  assert_eq!(negative, Err(StreamError::InvalidRoute { route: -1, partition_count: 2 }));

  let out_of_range = hub.route_with(2_u32, |_| 2);
  assert_eq!(out_of_range, Err(StreamError::InvalidRoute { route: 2, partition_count: 2 }));
}

#[test]
fn partition_hub_route_with_partitioner_does_not_invoke_callback_without_active_consumer() {
  let hub = PartitionHub::new(2);
  let mut called = false;

  let result = hub.route_with(1_u32, |_| {
    called = true;
    0
  });

  assert_eq!(result, Err(StreamError::WouldBlock));
  assert!(!called);
}
