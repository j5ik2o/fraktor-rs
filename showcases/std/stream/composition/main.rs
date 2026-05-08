use std::time::Duration;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::kernel::{actor::setup::ActorSystemConfig, system::ActorSystem};
use fraktor_stream_core_rs::core::{
  dsl::{Flow, Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
};

fn main() {
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("materializer start");
  let graph = Source::from_array([1_u32, 2])
    .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
    .into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let values = materialized.materialized().wait_blocking(&StdBlocker::new()).expect("stream should succeed");
  assert_eq!(values, vec![1, 2, 3, 4]);
  println!("stream_composition collected values: {values:?}");
  materializer.shutdown().expect("materializer shutdown");
}
