use std::time::Duration;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::kernel::{actor::setup::ActorSystemConfig, system::ActorSystem};
use fraktor_stream_core_rs::core::{
  dsl::{Flow, GraphDsl, GraphDslBuilder, Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight, StreamNotUsed},
};

fn main() {
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::noop_with_config(config).expect("actor system");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("materializer start");
  let flow = GraphDsl::create_flow(|builder: &mut GraphDslBuilder<u32, u32, StreamNotUsed>| {
    builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 10)).expect("add flow");
  });
  let graph = Source::from_array([1_u32, 2, 3]).via(flow).into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let values = materialized.materialized().wait_blocking(&StdBlocker::new()).expect("stream should succeed");
  assert_eq!(values, vec![11, 12, 13]);
  println!("stream_graphs collected values: {values:?}");
  materializer.shutdown().expect("materializer shutdown");
}
