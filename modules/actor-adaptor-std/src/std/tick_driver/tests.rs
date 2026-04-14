extern crate std;

use core::time::Duration;
use std::sync::mpsc;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use super::StdTickDriver;
use crate::std::StdBlocker;

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

fn assert_shutdown_completes(system: ActorSystem, operation: &str) {
  let termination = system.when_terminated();
  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());

  let (done_tx, done_rx) = mpsc::channel();
  std::thread::spawn(move || {
    drop(system);
    done_tx.send(()).expect("send shutdown completion");
  });

  done_rx
    .recv_timeout(Duration::from_secs(5))
    .unwrap_or_else(|_| panic!("{operation}: timed out waiting for StdTickDriver shutdown completion"));
}

#[test]
fn std_tick_driver_boots_actor_system() {
  let driver = StdTickDriver::new(Duration::from_millis(10));
  let system = build_system_with_driver(driver);
  assert_shutdown_completes(system, "StdTickDriver::new");
}

#[test]
fn std_tick_driver_default_boots_actor_system() {
  let system = build_system_with_driver(StdTickDriver::default());
  assert_shutdown_completes(system, "StdTickDriver::default");
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
