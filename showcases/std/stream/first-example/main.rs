#![cfg(not(target_os = "none"))]

use fraktor_showcases_std::support;
use fraktor_stream_core_rs::core::{
  dsl::{Sink, Source},
  materialization::KeepRight,
};

fn main() {
  let mut materializer = support::start_materializer();
  let graph = Source::single(41_u32).map(|value| value + 1).into_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let result = support::drive_until_ready(materialized.materialized(), 64)
    .expect("stream should complete")
    .expect("stream should succeed");
  assert_eq!(result, 42);
  materializer.shutdown().expect("materializer shutdown");
}
