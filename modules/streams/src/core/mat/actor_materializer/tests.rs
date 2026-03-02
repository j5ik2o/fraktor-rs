use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContext},
  error::ActorError,
  messaging::AnyMessageView,
  props::Props,
  scheduler::{
    SchedulerConfig,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{ActorSystem, ActorSystemConfig, remote::RemotingConfig},
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  Completion, KeepRight, StreamError,
  lifecycle::StreamState,
  mat::{ActorMaterializerConfig, ActorMaterializerGeneric},
  stage::{Sink, Source},
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  ActorSystem::new_with_config(&props, &config).expect("system should build")
}

#[test]
fn start_fails_without_actor_system() {
  let mut materializer =
    ActorMaterializerGeneric::<NoStdToolbox>::new_without_system(ActorMaterializerConfig::default());
  let result = materializer.start();
  assert!(matches!(result, Err(StreamError::ActorSystemMissing)));
}

#[test]
fn materialize_requires_start() {
  let system = build_system();
  let mut materializer = ActorMaterializerGeneric::<NoStdToolbox>::new(system, ActorMaterializerConfig::default());
  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);
  assert!(matches!(result, Err(StreamError::MaterializerNotStarted)));
}

#[test]
fn actor_materializer_drives_stream() {
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("manual controller").clone();
  let mut materializer = ActorMaterializerGeneric::<NoStdToolbox>::new(
    system,
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");
  let graph =
    Source::single(1_u32).map(|value| value + 1).to_mat(Sink::fold(0_u32, |acc, value| acc + value), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..5 {
    controller.inject_and_drive(1);
  }
  assert_eq!(materialized.handle().state(), StreamState::Completed);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(2)));
}

#[test]
fn shutdown_blocks_materialize() {
  let system = build_system();
  let mut materializer = ActorMaterializerGeneric::<NoStdToolbox>::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.shutdown().expect("shutdown");
  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);
  assert!(matches!(result, Err(StreamError::MaterializerStopped)));
}

#[test]
fn start_with_remoting_config() {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let remoting = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(2552);
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_remoting_config(remoting);
  let system = ActorSystem::new_with_config(&props, &config).expect("system should build");
  let mut materializer = ActorMaterializerGeneric::<NoStdToolbox>::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
}
