use std::time::Duration;

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{actor::setup::ActorSystemConfig, system::ActorSystem};
use fraktor_stream_core_kernel_rs::{
  dsl::{Flow, Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
};

fn main() -> Result<(), String> {
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_noop_guardian(config)
    .map_err(|error| format!("actor system creation failed: {error:?}"))?;
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().map_err(|error| format!("materializer start failed: {error}"))?;
  let graph = Source::from_array([1_u32, 2])
    .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
    .into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).map_err(|error| format!("materialization failed: {error}"))?;
  let values =
    materialized.materialized().wait_blocking(&StdBlocker::new()).map_err(|error| format!("stream failed: {error}"))?;
  assert_eq!(values, vec![1, 2, 3, 4]);
  println!("stream_composition collected values: {values:?}");
  materializer.shutdown().map_err(|error| format!("materializer shutdown failed: {error}"))?;
  Ok(())
}
