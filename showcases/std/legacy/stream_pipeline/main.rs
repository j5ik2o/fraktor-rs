//! Stream pipeline with Source, Map, FlatMapConcat, Fold, and Sink.
//!
//! Demonstrates three pipeline patterns in one example:
//! - Part 1: Source → Map → Sink::head (minimal pipeline)
//! - Part 2: Source → FlatMapConcat → Fold (aggregation)
//! - Part 3: Source → Map → filter via Option → Fold (map-and-filter)
//!
//! Run with: `cargo run -p fraktor-showcases-std --example stream_pipeline`

use std::time::Duration;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::kernel::{actor::setup::ActorSystemConfig, system::ActorSystem};
use fraktor_stream_core_rs::core::{
  dsl::{Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
};

fn main() {
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system");
  let mut mat =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  mat.start().expect("materializer start");
  let blocker = StdBlocker::new();

  // Part 1: Source → Map → Sink::head
  println!("=== Part 1: minimal pipeline (map + head) ===");
  let graph = Source::single(41_u32).map(|v| v + 1).into_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut mat).expect("run");
  match materialized.materialized().wait_blocking(&blocker) {
    | Ok(value) => println!("result: {value}"),
    | Err(error) => println!("failed: {error}"),
  }

  // Part 2: Source → FlatMapConcat → Fold
  println!("\n=== Part 2: flat_map_concat + fold (aggregation) ===");
  let graph = Source::single(5_u32)
    .flat_map_concat(|v| Source::single(v + 3))
    .into_mat(Sink::fold(10_u32, |acc, v| acc + v), KeepRight);
  let materialized = graph.run(&mut mat).expect("run");
  match materialized.materialized().wait_blocking(&blocker) {
    | Ok(sum) => println!("fold result: {sum}"),
    | Err(error) => println!("failed: {error}"),
  }

  // Part 3: Source → Map → filter via Option → Fold
  println!("\n=== Part 3: map + filter + fold ===");
  let graph = Source::single(9_u32).map(|v| v * 2).map(|v| if v >= 10 { Some(v) } else { None }).into_mat(
    Sink::fold(Vec::<u32>::new(), |mut acc, v| {
      if let Some(value) = v {
        acc.push(value);
      }
      acc
    }),
    KeepRight,
  );
  let materialized = graph.run(&mut mat).expect("run");
  match materialized.materialized().wait_blocking(&blocker) {
    | Ok(values) => println!("filtered values: {values:?}"),
    | Err(error) => println!("failed: {error}"),
  }

  mat.shutdown().expect("materializer shutdown");
}
