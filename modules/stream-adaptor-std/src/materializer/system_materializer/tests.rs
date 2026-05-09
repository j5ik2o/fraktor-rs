extern crate std;

use core::time::Duration;

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_stream_core_rs::{
  dsl::{Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig, KeepRight},
};

use super::SystemMaterializer;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  ActorSystem::create_from_props(&props, config).expect("system should build")
}

fn build_running_system_materializer() -> SystemMaterializer {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  SystemMaterializer::new(materializer)
}

#[test]
fn stream_snapshots_should_be_empty_when_no_stream_materialized() {
  let system_materializer = build_running_system_materializer();

  let snapshots = system_materializer.stream_snapshots();

  assert!(snapshots.is_empty());
}

#[test]
fn stream_snapshots_should_delegate_to_underlying_materializer() {
  let mut system_materializer = build_running_system_materializer();
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(system_materializer.materializer_mut()).expect("materialize");

  let snapshots = system_materializer.stream_snapshots();

  assert_eq!(snapshots.len(), 1);
}
