//! Shared wrapper for SchedulerContext providing thread-safe access.

use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{
  SchedulerBackedDelayProvider, SchedulerConfig, SchedulerContext, TaskRunSummary,
  tick_driver::{AutoDriverMetadata, TickDriverKind, TickDriverMetadata},
};
use crate::core::{
  event_stream::{EventStreamGeneric, TickDriverSnapshot},
  scheduler::Scheduler,
};

/// Shared wrapper that provides thread-safe access to a [`SchedulerContext`].
///
/// This adapter wraps a context in a `ToolboxMutex`, allowing it to be shared
/// across multiple owners while satisfying the `&mut self` requirement of
/// mutable `SchedulerContext` methods.
pub struct SchedulerContextSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<SchedulerContext<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for SchedulerContextSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SchedulerContextSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided context.
  #[must_use]
  pub fn new(context: SchedulerContext<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(context)) }
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

  /// Returns a reference to the inner shared mutex.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<ToolboxMutex<SchedulerContext<TB>, TB>> {
    &self.inner
  }

  /// Returns a clone of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> ArcShared<ToolboxMutex<Scheduler<TB>, TB>> {
    self.inner.lock().scheduler()
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.inner.lock().delay_provider()
  }

  /// Returns the event stream associated with this scheduler.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> {
    self.inner.lock().event_stream()
  }

  /// Returns the last recorded driver metadata.
  #[must_use]
  pub fn driver_metadata(&self) -> Option<TickDriverMetadata> {
    self.inner.lock().driver_metadata()
  }

  /// Returns the last recorded auto driver metadata.
  #[must_use]
  pub fn auto_driver_metadata(&self) -> Option<AutoDriverMetadata> {
    self.inner.lock().auto_driver_metadata()
  }

  /// Returns the last published driver snapshot.
  #[must_use]
  pub fn driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.inner.lock().driver_snapshot()
  }

  /// Records driver metadata (internal use).
  pub(crate) fn record_driver_metadata(
    &self,
    kind: TickDriverKind,
    resolution: Duration,
    metadata: TickDriverMetadata,
    auto: Option<AutoDriverMetadata>,
  ) {
    self.inner.lock().record_driver_metadata(kind, resolution, metadata, auto);
  }

  /// Publishes a driver warning (test support).
  #[cfg(any(test, feature = "test-support"))]
  pub(crate) fn publish_driver_warning(&self, message: &str) {
    self.inner.lock().publish_driver_warning(message);
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&mut self) -> TaskRunSummary {
    self.inner.lock().shutdown()
  }
}

/// Type alias for [`SchedulerContextSharedGeneric`] using the default [`NoStdToolbox`].
pub type SchedulerContextShared = SchedulerContextSharedGeneric<NoStdToolbox>;
