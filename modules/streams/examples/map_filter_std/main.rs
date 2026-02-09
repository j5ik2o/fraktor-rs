//! Basic map-and-filter style transformation using `Option` as a filter carrier.

#[path = "../std_materializer_support.rs"]
mod std_materializer_support;

use std::vec::Vec;

use fraktor_streams_rs::core::{KeepRight, Sink, Source};

fn main() {
  let (mut materializer, driver) = std_materializer_support::start_materializer();
  let graph =
    Source::single(9_u32).map(|value| value * 2).map(|value| if value >= 10 { Some(value) } else { None }).to_mat(
      Sink::fold(Vec::<u32>::new(), |mut acc, value| {
        if let Some(value) = value {
          acc.push(value);
        }
        acc
      }),
      KeepRight,
    );
  let materialized = graph.run(&mut materializer).expect("run");
  let completion = std_materializer_support::drive_until_ready(&driver, materialized.materialized(), 8);

  match completion {
    | Some(Ok(values)) => println!("map/filter values: {values:?}"),
    | Some(Err(error)) => println!("map/filter failed: {error}"),
    | None => println!("map/filter stream did not complete in the allotted ticks"),
  }

  materializer.shutdown().expect("materializer shutdown");
}
