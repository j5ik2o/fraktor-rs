#![cfg(not(target_os = "none"))]

use std::{
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, setup::ActorSystemConfig},
  system::ActorSystem,
};
use fraktor_stream_core_rs::core::{
  OverflowStrategy, ThrottleMode,
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
  let graph = Source::from_array([1_u32, 2, 3])
    .via(Flow::new().buffer(2, OverflowStrategy::Backpressure).expect("buffer"))
    .via(Flow::new().throttle(2, ThrottleMode::Shaping).expect("throttle"))
    .into_mat(Sink::collect(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let deadline = Instant::now() + Duration::from_millis(256);
  let values = loop {
    match materialized.materialized().poll() {
      | Completion::Ready(result) => break result.expect("stream should succeed"),
      | Completion::Pending if Instant::now() < deadline => thread::sleep(Duration::from_millis(1)),
      | Completion::Pending => panic!("stream should complete"),
    }
  };
  assert_eq!(values, vec![1, 2, 3]);
  materializer.shutdown().expect("materializer shutdown");
}
