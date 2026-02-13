//! Minimal Source-to-Sink pipeline with actor-backed materialization.

#[path = "../std_materializer_support.rs"]
mod std_materializer_support;

use fraktor_streams_rs::core::{
  KeepRight,
  stage::{Sink, Source},
};

fn main() {
  let (mut materializer, driver) = std_materializer_support::start_materializer();
  let graph = Source::single(41_u32).map(|value| value + 1).to_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let completion = std_materializer_support::drive_until_ready(&driver, materialized.materialized(), 8);

  match completion {
    | Some(Ok(value)) => println!("minimal pipeline result: {value}"),
    | Some(Err(error)) => println!("minimal pipeline failed: {error}"),
    | None => println!("minimal pipeline did not complete in the allotted ticks"),
  }

  materializer.shutdown().expect("materializer shutdown");
}
