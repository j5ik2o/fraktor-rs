//! Scheduler runtime container shared across the actor system.

#[cfg(any(test, feature = "test-support"))]
use alloc::borrow::ToOwned;
use core::time::Duration;

#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_rs::core::time::{MonotonicClock, TimerInstant};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{
    ArcShared,
    sync_mutex_like::{SpinSyncMutex, SyncMutexLike},
  },
};

use super::{
  Scheduler, SchedulerBackedDelayProvider, SchedulerConfig, TaskRunSummary,
  tick_driver::{AutoDriverMetadata, TickDriverKind, TickDriverMetadata},
};
use crate::core::event_stream::{EventStreamEvent, EventStreamGeneric, TickDriverSnapshot};
#[cfg(any(test, feature = "test-support"))]
use crate::core::logging::{LogEvent, LogLevel};

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct SchedulerContext<TB: RuntimeToolbox + 'static> {
  scheduler:       ArcShared<ToolboxMutex<Scheduler<TB>, TB>>,
  provider:        SchedulerBackedDelayProvider<TB>,
  event_stream:    ArcShared<EventStreamGeneric<TB>>,
  driver_snapshot: SpinSyncMutex<Option<TickDriverSnapshot>>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerContext<TB> {
  /// Creates a service from the provided toolbox and configuration.
  #[must_use]
  pub fn new(toolbox: TB, config: SchedulerConfig) -> Self {
    Self::with_event_stream(toolbox, config, ArcShared::new(EventStreamGeneric::default()))
  }

  /// Creates a service with the specified event stream handle.
  #[must_use]
  pub fn with_event_stream(
    toolbox: TB,
    config: SchedulerConfig,
    event_stream: ArcShared<EventStreamGeneric<TB>>,
  ) -> Self {
    let scheduler = Scheduler::new(toolbox, config);
    let mutex = <<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(scheduler);
    let shared = ArcShared::new(mutex);
    let provider = SchedulerBackedDelayProvider::new(shared.clone());
    Self { scheduler: shared, provider, event_stream, driver_snapshot: SpinSyncMutex::new(None) }
  }

  /// Returns a clone of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> ArcShared<ToolboxMutex<Scheduler<TB>, TB>> {
    self.scheduler.clone()
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.provider.clone()
  }

  /// Returns the event stream associated with this scheduler.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.event_stream.clone()
  }

  /// Returns the last recorded driver metadata.
  #[must_use]
  pub fn driver_metadata(&self) -> Option<TickDriverMetadata> {
    self.driver_snapshot.lock().as_ref().map(|snapshot| snapshot.metadata.clone())
  }

  /// Returns the last recorded auto driver metadata.
  #[must_use]
  pub fn auto_driver_metadata(&self) -> Option<AutoDriverMetadata> {
    self.driver_snapshot.lock().as_ref().and_then(|snapshot| snapshot.auto.clone())
  }

  /// Returns the last published driver snapshot.
  #[must_use]
  pub fn driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.driver_snapshot.lock().clone()
  }

  pub(crate) fn record_driver_metadata(
    &self,
    kind: TickDriverKind,
    resolution: Duration,
    metadata: TickDriverMetadata,
    auto: Option<AutoDriverMetadata>,
  ) {
    let snapshot = TickDriverSnapshot::new(metadata, kind, resolution, auto);
    *self.driver_snapshot.lock() = Some(snapshot.clone());
    self.event_stream.publish(&EventStreamEvent::TickDriver(snapshot));
  }

  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn publish_driver_warning(&self, message: &str) {
    let timestamp = self.current_timestamp();
    let event = EventStreamEvent::Log(LogEvent::new(LogLevel::Warn, message.to_owned(), timestamp, None));
    self.event_stream.publish(&event);
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&self) -> TaskRunSummary {
    let scheduler = self.scheduler.clone();
    let mut guard = scheduler.lock();
    guard.shutdown_with_tasks()
  }

  #[cfg(any(test, feature = "test-support"))]
  fn current_timestamp(&self) -> Duration {
    let scheduler = self.scheduler();
    let guard = scheduler.lock();
    instant_to_duration(guard.toolbox().clock().now())
  }
}

#[cfg(any(test, feature = "test-support"))]
fn instant_to_duration(instant: TimerInstant) -> Duration {
  let nanos = instant.resolution().as_nanos().saturating_mul(u128::from(instant.ticks()));
  Duration::from_nanos(nanos.min(u64::MAX as u128) as u64)
}
