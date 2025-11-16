//! Tokio-based tick driver implementations for std runtimes.

use std::time::Duration;

use fraktor_actor_core_rs::{
  event_stream::EventStreamGeneric,
  scheduler::{
    AutoProfileKind, TickDriverAutoLocator, TickDriverAutoLocatorRef, TickDriverConfig, TickDriverError,
    TickDriverFactoryRef,
  },
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
  ///
  /// # Panics
  ///
  /// Panics if no Tokio runtime handle is available in the current context.
  #[must_use]
  pub fn tokio_auto(resolution: Duration) -> TickDriverFactoryRef<StdToolbox> {
    let handle = Handle::try_current().expect("Tokio runtime handle unavailable");
    Self::tokio_with_handle(handle, resolution)
  }

  /// Creates a ready-to-use tick driver configuration for Tokio quickstart flows.
  #[must_use]
  pub fn tokio_quickstart() -> TickDriverConfig<StdToolbox> {
    Self::tokio_quickstart_with_resolution(Duration::from_millis(10))
  }

  /// Creates a Tokio quickstart configuration with custom resolution.
  #[must_use]
  pub fn tokio_quickstart_with_resolution(resolution: Duration) -> TickDriverConfig<StdToolbox> {
    TickDriverConfig::auto_with_factory(Self::tokio_auto(resolution))
  }

  /// Builds a factory using the provided Tokio runtime handle.
  #[must_use]
  pub fn tokio_with_handle(handle: Handle, resolution: Duration) -> TickDriverFactoryRef<StdToolbox> {
    ArcShared::new(TokioIntervalDriverFactory::new(handle, resolution))
  }

  /// Builds a factory that also publishes metrics to the provided event stream.
  ///
  /// # Panics
  ///
  /// Panics if no Tokio runtime handle is available in the current context.
  #[must_use]
  pub fn tokio_auto_with_event_stream(
    resolution: Duration,
    event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
    interval: Duration,
  ) -> TickDriverFactoryRef<StdToolbox> {
    let handle = Handle::try_current().expect("Tokio runtime handle unavailable");
    Self::tokio_with_handle_and_event_stream(handle, resolution, event_stream, interval)
  }

  /// Builds a factory with explicit handle and metrics publishing.
  #[must_use]
  pub fn tokio_with_handle_and_event_stream(
    handle: Handle,
    resolution: Duration,
    event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
    interval: Duration,
  ) -> TickDriverFactoryRef<StdToolbox> {
    ArcShared::new(TokioIntervalDriverFactory::new(handle, resolution).with_metrics(event_stream, interval))
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

  fn profile(&self) -> AutoProfileKind {
    AutoProfileKind::Tokio
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Mutex;

  use fraktor_actor_core_rs::{
    event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber},
    scheduler::{SchedulerConfig, SchedulerContext, TickDriverBootstrap, TickDriverConfig, TickDriverKind},
  };
  use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};

  use super::*;

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  #[allow(clippy::expect_used)]
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

    TickDriverBootstrap::shutdown(runtime.driver());
  }

  struct RecordingSubscriber {
    events: Mutex<Vec<EventStreamEvent<StdToolbox>>>,
  }

  impl RecordingSubscriber {
    fn new() -> Self {
      Self { events: Mutex::new(Vec::new()) }
    }

    #[allow(clippy::expect_used)]
    fn snapshot(&self) -> Vec<EventStreamEvent<StdToolbox>> {
      self.events.lock().expect("lock").clone()
    }
  }

  impl EventStreamSubscriber<StdToolbox> for RecordingSubscriber {
    #[allow(clippy::expect_used)]
    fn on_event(&self, event: &EventStreamEvent<StdToolbox>) {
      self.events.lock().expect("lock").push(event.clone());
    }
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  #[allow(clippy::expect_used)]
  async fn tokio_interval_driver_publishes_tick_metrics_events() {
    let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
    let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
    let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
    let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

    let factory = StdTickDriverConfig::tokio_auto_with_event_stream(
      Duration::from_millis(5),
      event_stream.clone(),
      Duration::from_millis(50),
    );
    let config = TickDriverConfig::auto_with_factory(factory);
    let ctx = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
    let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

    tokio::time::sleep(Duration::from_millis(120)).await;

    TickDriverBootstrap::shutdown(runtime.driver());

    let events = subscriber_impl.snapshot();
    assert!(
      events
        .iter()
        .any(|event| matches!(event, EventStreamEvent::SchedulerTick(metrics) if metrics.enqueued_total() > 0))
    );
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  #[allow(clippy::expect_used)]
  async fn tokio_quickstart_helper_provisions_driver() {
    let config = StdTickDriverConfig::tokio_quickstart();
    let ctx = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
    let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

    tokio::time::sleep(Duration::from_millis(40)).await;

    let resolution = ctx.scheduler().lock().config().resolution();
    let now = TimerInstant::from_ticks(1, resolution);
    let metrics = runtime.feed().expect("feed").snapshot(now, TickDriverKind::Auto);
    assert!(metrics.enqueued_total() > 0);

    TickDriverBootstrap::shutdown(runtime.driver());
  }
}
