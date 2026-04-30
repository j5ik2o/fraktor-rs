#![cfg(not(target_os = "none"))]

use fraktor_stream_core_rs::core::dsl::Source;

fn main() {
  let values = Source::from_array([1_u32, 2, 3])
    .map(|value| value * 2)
    .flat_map_concat(|value| Source::single(value + 1))
    .collect_values()
    .expect("collect values");
  assert_eq!(values, vec![3, 5, 7]);
}
