#![cfg(not(target_os = "none"))]

use fraktor_stream_core_rs::core::{
  OverflowStrategy, ThrottleMode,
  dsl::{Flow, Source},
};

fn main() {
  let values = Source::from_array([1_u32, 2, 3])
    .via(Flow::new().buffer(2, OverflowStrategy::Backpressure).expect("buffer"))
    .via(Flow::new().throttle(2, ThrottleMode::Shaping).expect("throttle"))
    .collect_values()
    .expect("collect values");
  assert_eq!(values, vec![1, 2, 3]);
}
