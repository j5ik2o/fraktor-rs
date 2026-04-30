#![cfg(not(target_os = "none"))]

use fraktor_showcases_std::support;
use fraktor_stream_core_rs::core::{
  OverflowStrategy, ThrottleMode,
  dsl::{Flow, Sink, Source},
  materialization::KeepRight,
};

fn main() {
  let mut materializer = support::start_materializer();
  let graph = Source::from_array([1_u32, 2, 3])
    .via(Flow::new().buffer(2, OverflowStrategy::Backpressure).expect("buffer"))
    .via(Flow::new().throttle(2, ThrottleMode::Shaping).expect("throttle"))
    .into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let values = support::drive_until_ready(materialized.materialized(), 256)
    .expect("stream should complete")
    .expect("stream should succeed");
  assert_eq!(values, vec![1, 2, 3]);
  materializer.shutdown().expect("materializer shutdown");
}
