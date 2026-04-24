use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::actor::scheduler::{
  SchedulerConfig, SchedulerContext,
  tick_driver::{
    SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind, TickExecutorSignal, TickFeed, TickFeedHandle,
  },
};

use super::StdTickDriver;

fn provision_inputs() -> (TickFeedHandle, SchedulerTickExecutor) {
  let config = SchedulerConfig::default();
  let context = SchedulerContext::new(config);
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::new(config.resolution(), 8, signal.clone());
  let executor = SchedulerTickExecutor::new(context.scheduler(), feed.clone(), signal);
  (feed, executor)
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
