//! Tick driver bootstrap orchestrates driver provisioning.

#[cfg(any(test, feature = "test-support"))]
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
#[cfg(any(test, feature = "test-support"))]
use core::time::Duration;

#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_core_rs::core::time::TimerInstant;
use fraktor_utils_core_rs::core::{sync::SharedAccess, time::MonotonicClock};

use super::{
  SchedulerTickExecutor, TickDriverBundle, TickDriverConfig, TickDriverControl, TickDriverControlShared,
  TickDriverError, TickDriverHandle, TickDriverMetadata, TickExecutorSignal, TickFeed,
};
#[cfg(any(test, feature = "test-support"))]
use super::{
  TickDriverKind,
  manual_test_driver::{ManualDriverControl, ManualTestDriver},
  tick_driver_trait::next_tick_driver_id,
};
#[cfg(any(test, feature = "test-support"))]
use crate::core::kernel::event::logging::{LogEvent, LogLevel};
use crate::core::kernel::{
  actor::scheduler::tick_driver::TickDriverProvisioningContext,
  event::stream::{EventStreamEvent, TickDriverSnapshot},
};

/// Bootstrapper responsible for wiring drivers into the scheduler context.
#[cfg(any(test, feature = "test-support"))]
pub struct TickDriverBootstrap;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) struct TickDriverBootstrap;

impl TickDriverBootstrap {
  /// Provisions the configured driver and returns the bundle with a snapshot.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] when driver provisioning fails.
  #[cfg(any(test, feature = "test-support"))]
  pub fn provision(
    config: &TickDriverConfig,
    ctx: &TickDriverProvisioningContext,
  ) -> Result<(TickDriverBundle, TickDriverSnapshot), TickDriverError> {
    Self::provision_impl(config, ctx)
  }

  #[cfg(not(any(test, feature = "test-support")))]
  pub(crate) fn provision(
    config: &TickDriverConfig,
    ctx: &TickDriverProvisioningContext,
  ) -> Result<(TickDriverBundle, TickDriverSnapshot), TickDriverError> {
    Self::provision_impl(config, ctx)
  }

  fn provision_impl(
    config: &TickDriverConfig,
    ctx: &TickDriverProvisioningContext,
  ) -> Result<(TickDriverBundle, TickDriverSnapshot), TickDriverError> {
    match config {
      #[cfg(any(test, feature = "test-support"))]
      | TickDriverConfig::ManualTest(driver) => Self::provision_manual(driver, ctx),
      | TickDriverConfig::Runtime { driver, executor_pump } => {
        let start_instant = {
          let scheduler = ctx.scheduler();
          scheduler.with_read(|s| s.clock().now())
        };
        let resolution = { driver.with_lock(|driver| driver.resolution()) };
        let capacity = {
          let scheduler = ctx.scheduler();
          scheduler.with_read(|s| s.config().profile().tick_buffer_quota())
        };
        let signal = TickExecutorSignal::new();
        let feed = TickFeed::new(resolution, capacity, signal.clone());
        let handle = { driver.with_lock(|driver| driver.start(feed.clone()))? };
        let auto_metadata =
          { executor_pump.with_lock(|executor_pump| executor_pump.auto_metadata(handle.id(), handle.resolution())) };
        let executor = SchedulerTickExecutor::new(ctx.scheduler(), feed.clone(), signal);
        let executor_control = {
          match executor_pump.with_lock(|executor_pump| executor_pump.spawn(executor)) {
            | Ok(control) => control,
            | Err(error) => {
              let mut handle = handle;
              handle.shutdown();
              return Err(error);
            },
          }
        };
        let handle = compose_runtime_handle(&handle, executor_control);
        let mut bundle = TickDriverBundle::new(handle, feed);
        if let Some(metadata) = auto_metadata.clone() {
          bundle = bundle.with_auto_metadata(metadata);
        }
        let handle = bundle.driver();
        let metadata = TickDriverMetadata::new(handle.id(), start_instant);
        let snapshot = TickDriverSnapshot::new(metadata, handle.kind(), handle.resolution(), auto_metadata);
        ctx.event_stream().publish(&EventStreamEvent::TickDriver(snapshot.clone()));
        Ok((bundle, snapshot))
      },
    }
  }

  #[cfg(any(test, feature = "test-support"))]
  fn provision_manual(
    driver: &ManualTestDriver,
    ctx: &TickDriverProvisioningContext,
  ) -> Result<(TickDriverBundle, TickDriverSnapshot), TickDriverError> {
    let scheduler = ctx.scheduler();
    let runner_enabled = scheduler.with_read(|s| s.config().runner_api_enabled());
    if !runner_enabled {
      publish_driver_warning(ctx, "manual tick driver was requested while runner API is disabled");
      return Err(TickDriverError::ManualDriverDisabled);
    }
    let (resolution, start_instant) = scheduler.with_read(|s| {
      let config = s.config();
      let instant = s.clock().now();
      (config.resolution(), instant)
    });
    driver.attach(ctx);
    let state = driver.state();
    let control: Box<dyn TickDriverControl> = Box::new(ManualDriverControl::new(state));
    let control = TickDriverControlShared::new(control);
    let handle = TickDriverHandle::new(next_tick_driver_id(), TickDriverKind::ManualTest, resolution, control);
    let controller = driver.controller();
    let bundle = TickDriverBundle::new_manual(handle.clone(), controller);
    let metadata = TickDriverMetadata::new(handle.id(), start_instant);
    let snapshot = TickDriverSnapshot::new(metadata, TickDriverKind::ManualTest, resolution, None);
    ctx.event_stream().publish(&EventStreamEvent::TickDriver(snapshot.clone()));
    Ok((bundle, snapshot))
  }
}

#[cfg(any(test, feature = "test-support"))]
fn publish_driver_warning(ctx: &TickDriverProvisioningContext, message: &str) {
  let timestamp = {
    let scheduler = ctx.scheduler();
    scheduler.with_read(|s| instant_to_duration(s.clock().now()))
  };
  let event = EventStreamEvent::Log(LogEvent::new(LogLevel::Warn, message.to_owned(), timestamp, None, None));
  ctx.event_stream().publish(&event);
}

#[cfg(any(test, feature = "test-support"))]
fn instant_to_duration(instant: TimerInstant) -> Duration {
  let nanos = instant.resolution().as_nanos().saturating_mul(u128::from(instant.ticks()));
  Duration::from_nanos(nanos.min(u64::MAX as u128) as u64)
}

struct CompositeTickDriverControl {
  driver_control:   TickDriverControlShared,
  executor_control: TickDriverControlShared,
}

impl CompositeTickDriverControl {
  const fn new(driver_control: TickDriverControlShared, executor_control: TickDriverControlShared) -> Self {
    Self { driver_control, executor_control }
  }
}

impl TickDriverControl for CompositeTickDriverControl {
  fn shutdown(&self) {
    self.driver_control.shutdown();
    self.executor_control.shutdown();
  }
}

fn compose_runtime_handle(handle: &TickDriverHandle, executor_control: Box<dyn TickDriverControl>) -> TickDriverHandle {
  let executor_control = TickDriverControlShared::new(executor_control);
  let control: Box<dyn TickDriverControl> =
    Box::new(CompositeTickDriverControl::new(handle.control(), executor_control));
  let control = TickDriverControlShared::new(control);
  TickDriverHandle::new(handle.id(), handle.kind(), handle.resolution(), control)
}
