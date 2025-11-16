//! Tokio-based tick driver implementations for std runtimes.

use std::time::Duration;

use fraktor_actor_core_rs::scheduler::{
  TickDriverAutoLocator, TickDriverAutoLocatorRef, TickDriverError, TickDriverFactoryRef,
};
use fraktor_utils_core_rs::sync::ArcShared;
use fraktor_utils_std_rs::runtime_toolbox::StdToolbox;
use tokio::runtime::Handle;

use crate::tick::tokio_impl::TokioIntervalDriverFactory;

mod tokio_impl;

/// Config helpers for std tick drivers.
pub struct StdTickDriverConfig;

impl StdTickDriverConfig {
  /// Builds a factory using the current Tokio runtime handle.
  #[must_use]
  pub fn tokio_auto(resolution: Duration) -> TickDriverFactoryRef<StdToolbox> {
    let handle = Handle::try_current().expect("Tokio runtime handle unavailable");
    Self::tokio_with_handle(handle, resolution)
  }

  /// Builds a factory using the provided Tokio runtime handle.
  #[must_use]
  pub fn tokio_with_handle(handle: Handle, resolution: Duration) -> TickDriverFactoryRef<StdToolbox> {
    ArcShared::new(TokioIntervalDriverFactory::new(handle, resolution))
  }
}

/// Auto locator that detects a Tokio runtime handle.
pub struct StdTokioAutoLocator;

impl TickDriverAutoLocator<StdToolbox> for StdTokioAutoLocator {
  fn detect(&self, _toolbox: &StdToolbox) -> Result<TickDriverFactoryRef<StdToolbox>, TickDriverError> {
    let handle = Handle::try_current().map_err(|_| TickDriverError::HandleUnavailable)?;
    Ok(StdTickDriverConfig::tokio_with_handle(handle, Duration::from_millis(10)))
  }

  fn default_ref() -> TickDriverAutoLocatorRef<StdToolbox>
  where
    Self: Sized, {
    ArcShared::new(Self)
  }
}

#[cfg(test)]
mod tests {
  use fraktor_actor_core_rs::scheduler::{
    SchedulerConfig, SchedulerContext, TickDriverBootstrap, TickDriverConfig, TickDriverKind,
  };
  use fraktor_utils_core_rs::time::TimerInstant;

  use super::*;

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn tokio_interval_driver_produces_ticks() {
    let factory = StdTickDriverConfig::tokio_auto(Duration::from_millis(5));
    let config = TickDriverConfig::auto_with_factory(factory);
    let ctx = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
    let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

    tokio::time::sleep(Duration::from_millis(20)).await;
    let resolution = ctx.scheduler().lock().config().resolution();
    let now = TimerInstant::from_ticks(1, resolution);
    let metrics = runtime.feed().expect("feed").snapshot(now, TickDriverKind::Auto);
    assert!(metrics.enqueued_total() > 0);

    TickDriverBootstrap::shutdown(runtime.driver().clone());
  }
}
