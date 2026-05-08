use std::time::Duration;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::kernel::{actor::setup::ActorSystemConfig, system::ActorSystem};
use fraktor_stream_core_rs::core::{
  dsl::{Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
};

fn main() {
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::noop_with_config(config).expect("actor system");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("materializer start");
  let graph = Source::single(41_u32).map(|value| value + 1).into_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let result = materialized.materialized().wait_blocking(&StdBlocker::new()).expect("stream should succeed");
  assert_eq!(result, 42);
  println!("stream_first_example result: {result}");
  materializer.shutdown().expect("materializer shutdown");
}
