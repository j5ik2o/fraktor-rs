#![cfg(not(target_os = "none"))]

use fraktor_showcases_std::support;
use fraktor_stream_core_rs::core::{
  dsl::{Sink, Source},
  materialization::KeepRight,
};

fn main() {
  let mut materializer = support::start_materializer();
  let graph = Source::from_array([1_u32, 2, 3])
    .map(|value| value * 2)
    .flat_map_concat(|value| Source::single(value + 1))
    .into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let values = support::drive_until_ready(materialized.materialized(), 128)
    .expect("stream should complete")
    .expect("stream should succeed");
  assert_eq!(values, vec![3, 5, 7]);
  materializer.shutdown().expect("materializer shutdown");
}
