extern crate std;

use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use super::StdTickDriver;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system_with_driver(driver: StdTickDriver) -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(driver).with_scheduler_config(scheduler);
  ActorSystem::create_with_config(&props, config).expect("system should build")
}

#[test]
fn std_tick_driver_boots_actor_system() {
  let driver = StdTickDriver::new(Duration::from_millis(10));
  let system = build_system_with_driver(driver);
  system.terminate().expect("terminate");
}

#[test]
fn std_tick_driver_default_boots_actor_system() {
  let system = build_system_with_driver(StdTickDriver::default());
  system.terminate().expect("terminate");
}

#[cfg(feature = "tokio-executor")]
mod tokio_tests {
  use core::time::Duration;

  use fraktor_actor_core_rs::core::kernel::{
    actor::{props::Props, scheduler::SchedulerConfig, setup::ActorSystemConfig},
    system::ActorSystem,
  };

  use super::GuardianActor;
  use crate::std::tick_driver::TokioTickDriver;

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn tokio_tick_driver_boots_actor_system() {
    let props = Props::from_fn(|| GuardianActor);
    let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
    let driver = TokioTickDriver::new(Duration::from_millis(10));
    let config = ActorSystemConfig::new(driver).with_scheduler_config(scheduler);
    let system = ActorSystem::create_with_config(&props, config).expect("system should build");
    system.terminate().expect("terminate");
  }
}
