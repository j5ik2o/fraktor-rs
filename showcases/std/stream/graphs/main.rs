#![cfg(not(target_os = "none"))]

use fraktor_showcases_std::support;
use fraktor_stream_core_rs::core::{
  dsl::{Flow, GraphDsl, GraphDslBuilder, Sink, Source},
  materialization::{KeepRight, StreamNotUsed},
};

fn main() {
  let mut materializer = support::start_materializer();
  let flow = GraphDsl::create_flow(|builder: &mut GraphDslBuilder<u32, u32, StreamNotUsed>| {
    builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 10)).expect("add flow");
  });
  let graph = Source::from_array([1_u32, 2, 3]).via(flow).into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let values = support::drive_until_ready(materialized.materialized(), 128)
    .expect("stream should complete")
    .expect("stream should succeed");
  assert_eq!(values, vec![11, 12, 13]);
  materializer.shutdown().expect("materializer shutdown");
}
