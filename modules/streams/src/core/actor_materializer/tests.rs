use core::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, SchedulerConfig, TickDriverConfig},
  system::{ActorSystemConfigGeneric, ActorSystemGeneric, RemotingConfig},
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  ActorMaterializerConfig, ActorMaterializerGeneric, Completion, KeepRight, Sink, Source, StreamError, StreamState,
};

struct GuardianActor;

impl Actor<NoStdToolbox> for GuardianActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystemGeneric<NoStdToolbox> {
  let props = PropsGeneric::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  ActorSystemGeneric::new_with_config(&props, &config).expect("system should build")
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
  let mut materializer = ActorMaterializerGeneric::new(system, ActorMaterializerConfig::default());
  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
  let result = materializer.materialize(graph);
  assert!(matches!(result, Err(StreamError::MaterializerNotStarted)));
}

#[test]
fn actor_materializer_drives_stream() {
  let system = build_system();
  let controller = system.tick_driver_runtime().manual_controller().expect("manual controller").clone();
  let mut materializer = ActorMaterializerGeneric::new(
    system,
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");
  let graph =
    Source::single(1_u32).map(|value| value + 1).to_mat(Sink::fold(0_u32, |acc, value| acc + value), KeepRight);
  let materialized = materializer.materialize(graph).expect("materialize");
  for _ in 0..5 {
    controller.inject_and_drive(1);
  }
  assert_eq!(materialized.handle().state(), StreamState::Completed);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(2)));
}

#[test]
fn shutdown_blocks_materialize() {
  let system = build_system();
  let mut materializer = ActorMaterializerGeneric::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.shutdown().expect("shutdown");
  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
  let result = materializer.materialize(graph);
  assert!(matches!(result, Err(StreamError::MaterializerStopped)));
}

#[test]
fn start_with_remoting_config() {
  let props = PropsGeneric::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let remoting = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(2552);
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_remoting_config(remoting);
  let system = ActorSystemGeneric::new_with_config(&props, &config).expect("system should build");
  let mut materializer = ActorMaterializerGeneric::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
}
