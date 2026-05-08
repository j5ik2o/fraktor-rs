extern crate std;

use core::{sync::atomic::AtomicBool, time::Duration};
use std::{
  sync::{Arc, mpsc},
  thread,
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    scheduler::{
      SchedulerConfig, SchedulerContext,
      tick_driver::{
        SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind, TickDriverStopper, TickExecutorSignal,
        TickFeed, TickFeedHandle,
      },
    },
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use super::{StdTickDriver, StdTickDriverStopper};
use crate::std::StdBlocker;

fn build_system_with_driver(driver: StdTickDriver) -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(driver).with_scheduler_config(scheduler);
  ActorSystem::create_with_noop_guardian(config).expect("system should build")
}

fn provision_inputs() -> (TickFeedHandle, SchedulerTickExecutor) {
  let config = SchedulerConfig::default();
  let context = SchedulerContext::new(config);
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::new(config.resolution(), 8, signal.clone());
  let executor = SchedulerTickExecutor::new(context.scheduler(), feed.clone(), signal);
  (feed, executor)
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
  let (feed, executor) = provision_inputs();
  let result = Box::new(StdTickDriver::new(Duration::ZERO)).provision(feed, executor);

  assert!(matches!(result, Err(TickDriverError::InvalidResolution)));
}

#[test]
fn std_tick_driver_provisions_and_stops_threads() {
  let resolution = Duration::from_millis(1);
  let (feed, executor) = provision_inputs();
  let provision = Box::new(StdTickDriver::new(resolution)).provision(feed, executor).expect("provision");

  assert_eq!(provision.resolution, resolution);
  assert_eq!(provision.kind, TickDriverKind::Std);
  provision.stopper.stop();
}

#[test]
fn std_tick_driver_emits_ticks_before_shutdown() {
  let (feed, executor) = provision_inputs();
  let signal = feed.signal();
  let provision =
    Box::new(StdTickDriver::new(Duration::from_nanos(1))).provision(feed.clone(), executor).expect("provision");

  let mut observed_tick = false;
  for _ in 0..1_000_000 {
    if signal.arm() {
      observed_tick = true;
      break;
    }
    thread::yield_now();
  }

  assert!(observed_tick, "tick driver did not signal any tick before yielding budget was exhausted");
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
    actor::{scheduler::SchedulerConfig, setup::ActorSystemConfig},
    system::ActorSystem,
  };

  use crate::std::tick_driver::TokioTickDriver;

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn tokio_tick_driver_boots_actor_system() {
    let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
    let driver = TokioTickDriver::new(Duration::from_millis(10));
    let config = ActorSystemConfig::new(driver).with_scheduler_config(scheduler);
    let system = ActorSystem::create_with_noop_guardian(config).expect("system should build");
    system.terminate().expect("terminate");
  }
}
