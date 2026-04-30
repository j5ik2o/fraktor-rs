#![cfg(not(target_os = "none"))]

use fraktor_stream_core_rs::core::{
  dsl::{Flow, GraphDsl, GraphDslBuilder, Source},
  materialization::StreamNotUsed,
};

fn main() {
  let flow = GraphDsl::create_flow(|builder: &mut GraphDslBuilder<u32, u32, StreamNotUsed>| {
    builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 10)).expect("add flow");
  });
  let values = Source::from_array([1_u32, 2, 3]).via(flow).collect_values().expect("collect values");
  assert_eq!(values, vec![11, 12, 13]);
}
