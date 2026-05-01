//! Stream pipeline with Source, Map, FlatMapConcat, Fold, and Sink.
//!
//! Demonstrates three pipeline patterns in one example:
//! - Part 1: Source → Map → Sink::head (minimal pipeline)
//! - Part 2: Source → FlatMapConcat → Fold (aggregation)
//! - Part 3: Source → Map → filter via Option → Fold (map-and-filter)
//!
//! Run with: `cargo run -p fraktor-showcases-std --example stream_pipeline`

use std::{thread, time::Duration};

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, setup::ActorSystemConfig},
  system::ActorSystem,
};
use fraktor_stream_core_rs::core::{
  dsl::{Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, Completion, KeepRight},
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn poll_until_ready<T: Clone>(
  completion: &fraktor_stream_core_rs::core::materialization::StreamFuture<T>,
  max_ticks: usize,
) -> Option<Result<T, fraktor_stream_core_rs::core::StreamError>> {
  for _ in 0..max_ticks {
    if let Completion::Ready(result) = completion.value() {
      return Some(result);
    }
    thread::sleep(Duration::from_millis(1));
  }
  None
}

#[allow(clippy::print_stdout)]
fn main() {
  let props = Props::from_fn(|| GuardianActor);
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_config(&props, config).expect("actor system");
  let mut mat =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  mat.start().expect("materializer start");

  // Part 1: Source → Map → Sink::head
  println!("=== Part 1: minimal pipeline (map + head) ===");
  let graph = Source::single(41_u32).map(|v| v + 1).into_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut mat).expect("run");
  let result = poll_until_ready(materialized.materialized(), 8);
  match result {
    | Some(Ok(value)) => println!("result: {value}"),
    | Some(Err(error)) => println!("failed: {error}"),
    | None => println!("did not complete in the allotted ticks"),
  }

  // Part 2: Source → FlatMapConcat → Fold
  println!("\n=== Part 2: flat_map_concat + fold (aggregation) ===");
  let graph = Source::single(5_u32)
    .flat_map_concat(|v| Source::single(v + 3))
    .into_mat(Sink::fold(10_u32, |acc, v| acc + v), KeepRight);
  let materialized = graph.run(&mut mat).expect("run");
  let result = poll_until_ready(materialized.materialized(), 8);
  match result {
    | Some(Ok(sum)) => println!("fold result: {sum}"),
    | Some(Err(error)) => println!("failed: {error}"),
    | None => println!("did not complete in the allotted ticks"),
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
  let result = poll_until_ready(materialized.materialized(), 8);
  match result {
    | Some(Ok(values)) => println!("filtered values: {values:?}"),
    | Some(Err(error)) => println!("failed: {error}"),
    | None => println!("did not complete in the allotted ticks"),
  }

  mat.shutdown().expect("materializer shutdown");
}
