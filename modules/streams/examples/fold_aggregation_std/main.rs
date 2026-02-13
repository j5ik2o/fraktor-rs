//! Fold-based aggregation that combines a transformed element into an accumulator.

#[path = "../std_materializer_support.rs"]
mod std_materializer_support;

use fraktor_streams_rs::core::{
  KeepRight,
  stage::{Sink, Source},
};

fn main() {
  let (mut materializer, driver) = std_materializer_support::start_materializer();
  let graph = Source::single(5_u32)
    .flat_map_concat(|value| Source::single(value + 3))
    .to_mat(Sink::fold(10_u32, |acc, value| acc + value), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let completion = std_materializer_support::drive_until_ready(&driver, materialized.materialized(), 8);

  match completion {
    | Some(Ok(sum)) => println!("fold aggregation result: {sum}"),
    | Some(Err(error)) => println!("fold aggregation failed: {error}"),
    | None => println!("fold aggregation stream did not complete in the allotted ticks"),
  }

  materializer.shutdown().expect("materializer shutdown");
}
