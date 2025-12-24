//! Tick driver bootstrap orchestrates driver provisioning.

#[cfg(any(test, feature = "test-support"))]
use alloc::{borrow::ToOwned, boxed::Box};

#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_rs::core::time::TimerInstant;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess, time::MonotonicClock};
#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_rs::core::{runtime_toolbox::SyncMutexFamily, sync::ArcShared};

#[cfg(any(test, feature = "test-support"))]
use super::TickDriverControl;
use super::{TickDriverConfig, TickDriverError, TickDriverHandleGeneric, TickDriverMetadata, TickDriverRuntime};
#[cfg(any(test, feature = "test-support"))]
use super::{
  TickDriverKind,
  manual_test_driver::{ManualDriverControl, ManualTestDriver},
  next_tick_driver_id,
};
#[cfg(any(test, feature = "test-support"))]
use crate::core::logging::{LogEvent, LogLevel};
use crate::core::{
  dispatch::scheduler::TickDriverProvisioningContext,
  event_stream::{EventStreamEvent, TickDriverSnapshot},
};

/// Bootstrapper responsible for wiring drivers into the scheduler context.
pub struct TickDriverBootstrap;

impl TickDriverBootstrap {
  /// Provisions the configured driver and returns the runtime assets with a snapshot.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] when driver provisioning fails.
  pub fn provision<TB: RuntimeToolbox>(
    config: &TickDriverConfig<TB>,
    ctx: &TickDriverProvisioningContext<TB>,
  ) -> Result<(TickDriverRuntime<TB>, TickDriverSnapshot), TickDriverError> {
    match config {
      #[cfg(any(test, feature = "test-support"))]
      | TickDriverConfig::ManualTest(driver) => Self::provision_manual(driver, ctx),
      | TickDriverConfig::Builder { builder } => {
        let start_instant = {
          let scheduler = ctx.scheduler();
          scheduler.with_read(|s| s.toolbox().clock().now())
        };
        let runtime = builder(ctx)?;
        let handle = runtime.driver();
        let metadata = TickDriverMetadata::new(handle.id(), start_instant);
        let auto_metadata = runtime.auto_metadata().cloned();
        let snapshot = TickDriverSnapshot::new(metadata, handle.kind(), handle.resolution(), auto_metadata);
        ctx.event_stream().publish(&EventStreamEvent::TickDriver(snapshot.clone()));
        Ok((runtime, snapshot))
      },
    }
  }

  #[cfg(any(test, feature = "test-support"))]
  fn provision_manual<TB: RuntimeToolbox>(
    driver: &ManualTestDriver<TB>,
    ctx: &TickDriverProvisioningContext<TB>,
  ) -> Result<(TickDriverRuntime<TB>, TickDriverSnapshot), TickDriverError> {
    let scheduler = ctx.scheduler();
    let runner_enabled = scheduler.with_read(|s| s.config().runner_api_enabled());
    if !runner_enabled {
      publish_driver_warning(ctx, "manual tick driver was requested while runner API is disabled");
      return Err(TickDriverError::ManualDriverDisabled);
    }
    let (resolution, start_instant) = scheduler.with_read(|s| {
      let config = s.config();
      let instant = s.toolbox().clock().now();
      (config.resolution(), instant)
    });
    driver.attach(ctx);
    let state = driver.state();
    let control: Box<dyn TickDriverControl> = Box::new(ManualDriverControl::new(state));
    let control = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(control));
    let handle = TickDriverHandleGeneric::new(next_tick_driver_id(), TickDriverKind::ManualTest, resolution, control);
    let controller = driver.controller();
    let runtime = TickDriverRuntime::new_manual(handle.clone(), controller);
    let metadata = TickDriverMetadata::new(handle.id(), start_instant);
    let snapshot = TickDriverSnapshot::new(metadata, TickDriverKind::ManualTest, resolution, None);
    ctx.event_stream().publish(&EventStreamEvent::TickDriver(snapshot.clone()));
    Ok((runtime, snapshot))
  }

  /// Shuts down the active driver handle.
  pub fn shutdown<TB: RuntimeToolbox>(handle: &mut TickDriverHandleGeneric<TB>) {
    handle.shutdown();
  }
}

#[cfg(any(test, feature = "test-support"))]
fn publish_driver_warning<TB: RuntimeToolbox>(ctx: &TickDriverProvisioningContext<TB>, message: &str) {
  let timestamp = {
    let scheduler = ctx.scheduler();
    scheduler.with_read(|s| instant_to_duration(s.toolbox().clock().now()))
  };
  let event = EventStreamEvent::Log(LogEvent::new(LogLevel::Warn, message.to_owned(), timestamp, None));
  ctx.event_stream().publish(&event);
}

#[cfg(any(test, feature = "test-support"))]
fn instant_to_duration(instant: TimerInstant) -> core::time::Duration {
  let nanos = instant.resolution().as_nanos().saturating_mul(u128::from(instant.ticks()));
  core::time::Duration::from_nanos(nanos.min(u64::MAX as u128) as u64)
}
