#![cfg(not(target_os = "none"))]

use std::{thread, time::Duration};

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, setup::ActorSystemConfig},
  system::ActorSystem,
};
use fraktor_stream_core_rs::core::{
  dsl::{Flow, Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, Completion, KeepRight},
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn main() {
  let props = Props::from_fn(|| GuardianActor);
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_config(&props, config).expect("actor system");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("materializer start");
  let graph = Source::from_array([1_u32, 2])
    .via(Flow::new().concat_lazy(Source::from_array([3_u32, 4])))
    .into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let mut values = None;
  for _ in 0..128 {
    match materialized.materialized().value() {
      | Completion::Ready(ready) => {
        values = Some(ready.expect("stream should succeed"));
        break;
      },
      | Completion::Pending => thread::sleep(Duration::from_millis(1)),
    }
  }
  let values = values.expect("stream should complete");
  assert_eq!(values, vec![1, 2, 3, 4]);
  materializer.shutdown().expect("materializer shutdown");
}
