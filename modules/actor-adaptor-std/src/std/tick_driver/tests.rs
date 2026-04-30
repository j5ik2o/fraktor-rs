extern crate std;

use core::{sync::atomic::AtomicBool, time::Duration};
use std::{
  sync::{Arc, mpsc},
  thread,
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::AnyMessageView,
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{TickDriver, TickDriverKind, TickDriverStopper},
    },
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use super::{StdTickDriver, StdTickDriverStopper};
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

#[test]
fn std_tick_driver_new_default_and_kind_expose_std_contract() {
  let driver = StdTickDriver::new(Duration::from_millis(3));
  assert_eq!(driver.kind(), TickDriverKind::Std);
  assert_eq!(StdTickDriver::default().kind(), TickDriverKind::Std);
}

#[test]
fn std_tick_driver_rejects_zero_resolution() {
  use fraktor_actor_core_rs::core::kernel::actor::scheduler::{
    SchedulerContext,
    tick_driver::{SchedulerTickExecutor, TickDriver, TickDriverError, TickExecutorSignal, TickFeed, TickFeedHandle},
  };

  fn provision_inputs() -> (TickFeedHandle, SchedulerTickExecutor) {
    let config = SchedulerConfig::default();
    let context = SchedulerContext::new(config);
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(config.resolution(), 8, signal.clone());
    let executor = SchedulerTickExecutor::new(context.scheduler(), feed.clone(), signal);
    (feed, executor)
  }

  let (feed, executor) = provision_inputs();
  let result = Box::new(StdTickDriver::new(Duration::ZERO)).provision(feed, executor);

  assert!(matches!(result, Err(TickDriverError::InvalidResolution)));
}

#[test]
fn std_tick_driver_provisions_and_stops_threads() {
  use fraktor_actor_core_rs::core::kernel::actor::scheduler::{
    SchedulerContext,
    tick_driver::{SchedulerTickExecutor, TickDriver, TickDriverKind, TickExecutorSignal, TickFeed, TickFeedHandle},
  };

  fn provision_inputs() -> (TickFeedHandle, SchedulerTickExecutor) {
    let config = SchedulerConfig::default();
    let context = SchedulerContext::new(config);
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(config.resolution(), 8, signal.clone());
    let executor = SchedulerTickExecutor::new(context.scheduler(), feed.clone(), signal);
    (feed, executor)
  }

  let resolution = Duration::from_millis(1);
  let (feed, executor) = provision_inputs();
  let provision = Box::new(StdTickDriver::new(resolution)).provision(feed, executor).expect("provision");

  assert_eq!(provision.resolution, resolution);
  assert_eq!(provision.kind, TickDriverKind::Std);
  provision.stopper.stop();
}

#[test]
fn std_tick_driver_emits_ticks_before_shutdown() {
  use fraktor_actor_core_rs::core::kernel::actor::scheduler::{
    SchedulerContext,
    tick_driver::{SchedulerTickExecutor, TickDriver, TickExecutorSignal, TickFeed, TickFeedHandle},
  };

  fn provision_inputs() -> (TickFeedHandle, SchedulerTickExecutor) {
    let config = SchedulerConfig::default();
    let context = SchedulerContext::new(config);
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(config.resolution(), 8, signal.clone());
    let executor = SchedulerTickExecutor::new(context.scheduler(), feed.clone(), signal);
    (feed, executor)
  }

  let (feed, executor) = provision_inputs();
  let provision =
    Box::new(StdTickDriver::new(Duration::from_millis(1))).provision(feed.clone(), executor).expect("provision");

  for _ in 0..10_000 {
    if feed.driver_active() {
      break;
    }
    thread::yield_now();
  }

  assert!(feed.driver_active(), "tick driver did not enqueue any tick before yielding budget was exhausted");
  provision.stopper.stop();
}

#[test]
fn std_tick_driver_stopper_absorbs_panicked_worker_threads() {
  let stopper = StdTickDriverStopper {
    running:     Arc::new(AtomicBool::new(true)),
    tick_thread: Some(thread::spawn(|| panic!("tick thread panic for coverage"))),
    exec_thread: Some(thread::spawn(|| panic!("executor thread panic for coverage"))),
  };

  Box::new(stopper).stop();
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
