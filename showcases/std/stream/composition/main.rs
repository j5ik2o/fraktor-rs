#![cfg(not(target_os = "none"))]

use fraktor_stream_core_rs::core::dsl::{Flow, Source};

fn main() {
  let values = Source::from_array([1_u32, 2])
    .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
    .collect_values()
    .expect("collect values");
  assert_eq!(values, vec![1, 2, 3, 4]);
}
