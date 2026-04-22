//! Tick driver bootstrap orchestrates driver provisioning.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::{sync::SharedAccess, time::MonotonicClock};

use super::{
  SchedulerTickExecutor, TickDriver, TickDriverBundle, TickDriverError, TickDriverMetadata, TickExecutorSignal,
  TickFeed, TickFeedHandle,
};
use crate::core::kernel::{
  actor::scheduler::tick_driver::{BootstrapProvisionResult, TickDriverProvisioningContext},
  event::stream::{EventStreamEvent, TickDriverSnapshot},
};

/// Bootstrapper responsible for wiring drivers into the scheduler context.
pub(crate) struct TickDriverBootstrap;

impl TickDriverBootstrap {
  /// Provisions a driver and returns the bundle, stopper, and snapshot.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] when driver provisioning fails.
  pub(crate) fn provision(
    driver: Box<dyn TickDriver>,
    ctx: &TickDriverProvisioningContext,
  ) -> Result<BootstrapProvisionResult, TickDriverError> {
    Self::provision_impl(driver, ctx)
  }

  fn provision_impl(
    driver: Box<dyn TickDriver>,
    ctx: &TickDriverProvisioningContext,
  ) -> Result<BootstrapProvisionResult, TickDriverError> {
    let (start_instant, capacity, resolution) = {
      let scheduler = ctx.scheduler();
      scheduler.with_read(|s| (s.clock().now(), s.config().profile().tick_buffer_quota(), s.config().resolution()))
    };
    let signal = TickExecutorSignal::new();
    let feed_handle: TickFeedHandle = TickFeed::new(resolution, capacity, signal.clone());
    let scheduler = ctx.scheduler();
    let executor = SchedulerTickExecutor::new(scheduler, feed_handle.clone(), signal);
    let provision = driver.provision(feed_handle.clone(), executor)?;
    let resolution = provision.resolution;
    feed_handle.set_resolution(resolution);
    let id = provision.id;
    let kind = provision.kind;
    let auto_metadata = provision.auto_metadata.clone();
    let stopper = provision.stopper;

    let bundle = if let Some(ref metadata) = auto_metadata {
      TickDriverBundle::new(id, kind, resolution, feed_handle).with_auto_metadata(metadata.clone())
    } else {
      TickDriverBundle::new(id, kind, resolution, feed_handle)
    };
    let metadata = TickDriverMetadata::new(id, start_instant);
    let snapshot = TickDriverSnapshot::new(metadata, kind, resolution, auto_metadata);
    ctx.event_stream().publish(&EventStreamEvent::TickDriver(snapshot.clone()));

    Ok(BootstrapProvisionResult { bundle, stopper, snapshot })
  }
}
