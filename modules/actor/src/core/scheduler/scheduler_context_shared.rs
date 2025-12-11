//! Shared wrapper for SchedulerContext providing thread-safe access.

#[cfg(any(test, feature = "test-support"))]
use alloc::borrow::ToOwned;
use core::time::Duration;

#[cfg(any(test, feature = "test-support"))]
use fraktor_utils_rs::core::time::{MonotonicClock, TimerInstant};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::{
  SchedulerBackedDelayProvider, SchedulerConfig, SchedulerContext, SchedulerSharedGeneric, TaskRunSummary,
  tick_driver::{AutoDriverMetadata, TickDriverKind, TickDriverMetadata},
};
use crate::core::event_stream::{EventStreamEvent, EventStreamGeneric, TickDriverSnapshot};
#[cfg(any(test, feature = "test-support"))]
use crate::core::logging::{LogEvent, LogLevel};

pub(crate) struct SchedulerContextHandle<TB: RuntimeToolbox + 'static> {
  scheduler:       SchedulerSharedGeneric<TB>,
  provider:        SchedulerBackedDelayProvider<TB>,
  event_stream:    ArcShared<EventStreamGeneric<TB>>,
  driver_snapshot: Option<TickDriverSnapshot>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerContextHandle<TB> {
  fn scheduler(&self) -> SchedulerSharedGeneric<TB> {
    self.scheduler.clone()
  }

  fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.provider.clone()
  }

  fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.event_stream.clone()
  }

  #[allow(dead_code)]
  fn driver_metadata(&self) -> Option<TickDriverMetadata> {
    self.driver_snapshot.as_ref().map(|snapshot| snapshot.metadata.clone())
  }

  #[allow(dead_code)]
  fn auto_driver_metadata(&self) -> Option<AutoDriverMetadata> {
    self.driver_snapshot.as_ref().and_then(|snapshot| snapshot.auto.clone())
  }

  #[allow(dead_code)]
  fn driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.driver_snapshot.clone()
  }

  #[allow(dead_code)]
  fn record_driver_metadata(
    &mut self,
    kind: TickDriverKind,
    resolution: Duration,
    metadata: TickDriverMetadata,
    auto: Option<AutoDriverMetadata>,
  ) {
    let snapshot = TickDriverSnapshot::new(metadata, kind, resolution, auto);
    self.driver_snapshot = Some(snapshot.clone());
    self.event_stream.publish(&EventStreamEvent::TickDriver(snapshot));
  }

  #[allow(dead_code)]
  #[cfg(any(test, feature = "test-support"))]
  fn publish_driver_warning(&self, message: &str) {
    let timestamp = self.current_timestamp();
    let event = EventStreamEvent::Log(LogEvent::new(LogLevel::Warn, message.to_owned(), timestamp, None));
    self.event_stream.publish(&event);
  }

  fn shutdown(&mut self) -> TaskRunSummary {
    self.scheduler.with_write(|s| s.shutdown_with_tasks())
  }

  #[cfg(any(test, feature = "test-support"))]
  fn current_timestamp(&self) -> Duration {
    let scheduler = self.scheduler();
    scheduler.with_write(|s| instant_to_duration(s.toolbox().clock().now()))
  }
}

/// Shared wrapper that provides thread-safe access to a [`SchedulerContext`].
///
/// Thin layer: lock and delegate to `SchedulerContextHandle`.
pub struct SchedulerContextSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<SchedulerContextHandle<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for SchedulerContextSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SchedulerContextSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided context.
  #[must_use]
  #[allow(clippy::needless_pass_by_value)]
  pub fn new(context: SchedulerContext<TB>) -> Self {
    let handle = SchedulerContextHandle {
      scheduler:       context.scheduler(),
      provider:        context.delay_provider(),
      event_stream:    context.event_stream(),
      driver_snapshot: context.driver_snapshot(),
    };
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(handle)) }
  }

  /// Creates a shared context from the provided toolbox and configuration.
  #[must_use]
  pub fn from_config(toolbox: TB, config: SchedulerConfig) -> Self {
    Self::new(SchedulerContext::new(toolbox, config))
  }

  /// Creates a shared context with the specified event stream handle.
  #[must_use]
  pub fn with_event_stream(
    toolbox: TB,
    config: SchedulerConfig,
    event_stream: ArcShared<EventStreamGeneric<TB>>,
  ) -> Self {
    Self::new(SchedulerContext::with_event_stream(toolbox, config, event_stream))
  }

  /// Returns a clone of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerSharedGeneric<TB> {
    self.with_read(|handle| handle.scheduler())
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.with_read(|handle| handle.delay_provider())
  }

  /// Returns the event stream associated with this scheduler.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.with_read(|handle| handle.event_stream())
  }

  /// Returns the last recorded driver metadata.
  #[must_use]
  pub fn driver_metadata(&self) -> Option<TickDriverMetadata> {
    self.with_read(|handle| handle.driver_metadata())
  }

  /// Returns the last recorded auto driver metadata.
  #[must_use]
  pub fn auto_driver_metadata(&self) -> Option<AutoDriverMetadata> {
    self.with_read(|handle| handle.auto_driver_metadata())
  }

  /// Returns the last published driver snapshot.
  #[must_use]
  pub fn driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.with_read(|handle| handle.driver_snapshot())
  }

  /// Records driver metadata (internal use).
  pub(crate) fn record_driver_metadata(
    &self,
    kind: TickDriverKind,
    resolution: Duration,
    metadata: TickDriverMetadata,
    auto: Option<AutoDriverMetadata>,
  ) {
    self.with_write(|handle| handle.record_driver_metadata(kind, resolution, metadata, auto));
  }

  /// Publishes a driver warning (test support).
  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn publish_driver_warning(&self, message: &str) {
    self.with_read(|handle| handle.publish_driver_warning(message));
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&self) -> TaskRunSummary {
    self.with_write(|handle| handle.shutdown())
  }
}

/// Type alias for [`SchedulerContextSharedGeneric`] using the default [`NoStdToolbox`].
pub type SchedulerContextShared = SchedulerContextSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> SharedAccess<SchedulerContextHandle<TB>> for SchedulerContextSharedGeneric<TB> {
  #[inline]
  fn with_read<R>(&self, f: impl FnOnce(&SchedulerContextHandle<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  #[inline]
  fn with_write<R>(&self, f: impl FnOnce(&mut SchedulerContextHandle<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

#[cfg(any(test, feature = "test-support"))]
fn instant_to_duration(instant: TimerInstant) -> Duration {
  let nanos = instant.resolution().as_nanos().saturating_mul(u128::from(instant.ticks()));
  Duration::from_nanos(nanos.min(u64::MAX as u128) as u64)
}
