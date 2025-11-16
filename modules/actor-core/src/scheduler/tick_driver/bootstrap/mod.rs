//! Tick driver bootstrap orchestrates driver provisioning.

use alloc::boxed::Box;

#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_core_rs::sync::ArcShared;
use fraktor_utils_core_rs::sync::sync_mutex_like::SyncMutexLike;

use super::{
  HardwareKind, TickDriver, TickDriverConfig, TickDriverError, TickDriverHandle, TickDriverRuntime, TickExecutorSignal,
  TickFeed, hardware_driver::HardwareTickDriver,
};
#[cfg(any(test, feature = "test-support"))]
use super::{manual_test_driver::{ManualDriverControl, ManualTestDriver}, TickDriverKind, next_tick_driver_id};
use crate::{RuntimeToolbox, scheduler::SchedulerContext};

/// Bootstrapper responsible for wiring drivers into the scheduler context.
pub struct TickDriverBootstrap;

impl TickDriverBootstrap {
  /// Provisions the configured driver and returns the runtime assets.
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
    let (resolution, capacity) = {
      let guard = scheduler.lock();
      let cfg = guard.config();
      (cfg.resolution(), cfg.profile().tick_buffer_quota())
    };
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let handle = driver.start(feed.clone())?;
    Ok(TickDriverRuntime::new(handle, feed))
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
    driver.attach(ctx);
    let state = driver.state();
    let control = ArcShared::new(ManualDriverControl::new(state));
    let resolution = ctx.scheduler().lock().config().resolution();
    let handle = TickDriverHandle::new(next_tick_driver_id(), TickDriverKind::ManualTest, resolution, control);
    let controller = driver.controller();
    Ok(TickDriverRuntime::new_manual(handle, controller))
  }

  /// Shuts down the active driver handle.
  pub fn shutdown(handle: TickDriverHandle) {
    handle.shutdown();
  }
}
