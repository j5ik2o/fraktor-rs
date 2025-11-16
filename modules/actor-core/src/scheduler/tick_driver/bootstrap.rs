//! Tick driver bootstrap orchestrates driver provisioning.

use alloc::boxed::Box;

#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_core_rs::sync::ArcShared;
use fraktor_utils_core_rs::{sync::sync_mutex_like::SyncMutexLike, time::MonotonicClock};

use super::{
  AutoDriverMetadata, AutoProfileKind, HardwareKind, TickDriver, TickDriverConfig, TickDriverError, TickDriverHandle,
  TickDriverMetadata, TickDriverRuntime, TickExecutorSignal, TickFeed, hardware_driver::HardwareTickDriver,
};
#[cfg(any(test, feature = "test-support"))]
use super::{
  TickDriverKind,
  manual_test_driver::{ManualDriverControl, ManualTestDriver},
  next_tick_driver_id,
};
use crate::{RuntimeToolbox, scheduler::SchedulerContext};

/// Bootstrapper responsible for wiring drivers into the scheduler context.
pub struct TickDriverBootstrap;

impl TickDriverBootstrap {
  /// Provisions the configured driver and returns the runtime assets.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] when driver provisioning fails.
  pub fn provision<TB: RuntimeToolbox>(
    config: &TickDriverConfig<TB>,
    ctx: &SchedulerContext<TB>,
  ) -> Result<TickDriverRuntime<TB>, TickDriverError> {
    #[cfg(any(test, feature = "test-support"))]
    if let TickDriverConfig::ManualTest(driver) = config {
      return Self::provision_manual(driver, ctx);
    }

    let driver = Self::resolve_driver(config, ctx)?;
    let scheduler = ctx.scheduler();
    let (resolution, capacity, start_instant) = {
      let guard = scheduler.lock();
      let cfg = guard.config();
      let instant = guard.toolbox().clock().now();
      (cfg.resolution(), cfg.profile().tick_buffer_quota(), instant)
    };
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let handle = driver.start(feed.clone())?;
    let runtime = TickDriverRuntime::new(handle.clone(), feed);
    let metadata = TickDriverMetadata::new(handle.id(), start_instant);
    let auto_metadata = match config {
      | TickDriverConfig::Auto(auto) => {
        let profile = auto.profile().unwrap_or(AutoProfileKind::Custom);
        Some(AutoDriverMetadata { profile, driver_id: handle.id(), resolution })
      },
      | _ => None,
    };
    ctx.record_driver_metadata(handle.kind(), resolution, metadata, auto_metadata);
    Ok(runtime)
  }

  fn resolve_driver<TB: RuntimeToolbox>(
    config: &TickDriverConfig<TB>,
    ctx: &SchedulerContext<TB>,
  ) -> Result<Box<dyn TickDriver<TB>>, TickDriverError> {
    match config {
      | TickDriverConfig::Auto(auto) => {
        if let Some(factory) = auto.factory() {
          return factory.build();
        }
        if let Some(locator) = auto.locator() {
          let scheduler = ctx.scheduler();
          let guard = scheduler.lock();
          let factory = locator.detect(guard.toolbox())?;
          return factory.build();
        }
        Err(TickDriverError::UnsupportedEnvironment)
      },
      | TickDriverConfig::Hardware { driver } => {
        let hardware = HardwareTickDriver::<TB>::new(*driver, HardwareKind::Custom);
        Ok(Box::new(hardware))
      },
      #[cfg(any(test, feature = "test-support"))]
      | TickDriverConfig::ManualTest(_) => Err(TickDriverError::UnsupportedEnvironment),
    }
  }

  #[cfg(any(test, feature = "test-support"))]
  fn provision_manual<TB: RuntimeToolbox>(
    driver: &ManualTestDriver<TB>,
    ctx: &SchedulerContext<TB>,
  ) -> Result<TickDriverRuntime<TB>, TickDriverError> {
    let scheduler = ctx.scheduler();
    let runner_enabled = {
      let guard = scheduler.lock();
      guard.config().runner_api_enabled()
    };
    if !runner_enabled {
      ctx.publish_driver_warning("manual tick driver was requested while runner API is disabled");
      return Err(TickDriverError::ManualDriverDisabled);
    }
    let (resolution, start_instant) = {
      let guard = scheduler.lock();
      let config = guard.config();
      let instant = guard.toolbox().clock().now();
      (config.resolution(), instant)
    };
    driver.attach(ctx);
    let state = driver.state();
    let control = ArcShared::new(ManualDriverControl::new(state));
    let handle = TickDriverHandle::new(next_tick_driver_id(), TickDriverKind::ManualTest, resolution, control);
    let controller = driver.controller();
    let runtime = TickDriverRuntime::new_manual(handle.clone(), controller);
    let metadata = TickDriverMetadata::new(handle.id(), start_instant);
    ctx.record_driver_metadata(TickDriverKind::ManualTest, resolution, metadata, None);
    Ok(runtime)
  }

  /// Shuts down the active driver handle.
  pub fn shutdown(handle: &TickDriverHandle) {
    handle.shutdown();
  }
}
