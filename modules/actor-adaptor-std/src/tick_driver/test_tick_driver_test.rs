use core::time::Duration;

use fraktor_actor_core_kernel_rs::actor::scheduler::{
  SchedulerConfig, SchedulerContext,
  tick_driver::{
    SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind, TickExecutorSignal, TickFeed, TickFeedHandle,
  },
};

use super::TestTickDriver;

fn provision_inputs() -> (TickFeedHandle, SchedulerTickExecutor) {
  let config = SchedulerConfig::default();
  let context = SchedulerContext::new(config);
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::new(config.resolution(), 8, signal.clone());
  let executor = SchedulerTickExecutor::new(context.scheduler(), feed.clone(), signal);
  (feed, executor)
}

#[test]
fn test_tick_driver_default_and_kind_expose_manual_contract() {
  let driver = TestTickDriver::default();

  assert_eq!(driver.kind(), TickDriverKind::Manual);
}

#[test]
fn test_tick_driver_rejects_zero_resolution() {
  let (feed, executor) = provision_inputs();
  let result = Box::new(TestTickDriver { resolution: Duration::ZERO }).provision(feed, executor);

  assert!(matches!(result, Err(TickDriverError::InvalidResolution)));
}

#[test]
fn test_tick_driver_provisions_and_stops_threads() {
  let resolution = Duration::from_millis(1);
  let (feed, executor) = provision_inputs();
  let provision = Box::new(TestTickDriver { resolution }).provision(feed, executor).expect("provision");

  assert_eq!(provision.resolution, resolution);
  assert_eq!(provision.kind, TickDriverKind::Manual);
  provision.stopper.stop();
}
