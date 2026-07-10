use std::{error::Error, time::Duration};

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{actor::setup::ActorSystemConfig, system::ActorSystem};
use fraktor_stream_core_kernel_rs::{
  dsl::{Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
};

fn main() -> Result<(), Box<dyn Error>> {
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_noop_guardian(config)?;
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start()?;
  let graph = Source::single(41_u32).map(|value| value + 1).into_mat(Sink::head(), KeepRight);
  let running = graph.run(&mut materializer)?;
  let result = running.materialized().wait_blocking(&StdBlocker::new())?;
  assert_eq!(result, 42);
  println!("stream_first_example result: {result}");
  materializer.shutdown()?;
  Ok(())
}
