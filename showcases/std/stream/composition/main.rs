#![cfg(not(target_os = "none"))]

use fraktor_showcases_std::support;
use fraktor_stream_core_rs::core::{
  dsl::{Flow, Sink, Source},
  materialization::KeepRight,
};

fn main() {
  let mut materializer = support::start_materializer();
  let graph = Source::from_array([1_u32, 2])
    .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
    .into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let values = support::drive_until_ready(materialized.materialized(), 128)
    .expect("stream should complete")
    .expect("stream should succeed");
  assert_eq!(values, vec![1, 2, 3, 4]);
  materializer.shutdown().expect("materializer shutdown");
}
