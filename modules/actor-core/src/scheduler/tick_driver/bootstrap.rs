//! Tick driver bootstrap orchestrates driver provisioning.

#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_core_rs::core::sync::ArcShared;
use fraktor_utils_core_rs::core::{sync::sync_mutex_like::SyncMutexLike, time::MonotonicClock};

use super::{TickDriverConfig, TickDriverError, TickDriverHandle, TickDriverMetadata, TickDriverRuntime};
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
    match config {
      #[cfg(any(test, feature = "test-support"))]
      | TickDriverConfig::ManualTest(driver) => Self::provision_manual(driver, ctx),
      | TickDriverConfig::Builder { builder } => {
        let start_instant = {
          let scheduler = ctx.scheduler();
          let guard = scheduler.lock();
          guard.toolbox().clock().now()
        };
        let runtime = builder(ctx)?;
        let handle = runtime.driver();
        let metadata = TickDriverMetadata::new(handle.id(), start_instant);
        let auto_metadata = runtime.auto_metadata().cloned();
        ctx.record_driver_metadata(handle.kind(), handle.resolution(), metadata, auto_metadata);
        Ok(runtime)
      },
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
